use canyon_connection::get_database_type_from_datasource_name;
use canyon_crud::{crud::Transaction, bounds::RowOperations, DatabaseType};
use std::collections::HashMap;
use std::fs;
use walkdir::WalkDir;

use crate::{QUERIES_TO_EXECUTE, constants};

/// Convenient struct that contains the necessary data and operations to implement
/// the `Canyon Memory`.
///
/// Canyon Memory it's just a convenient way of relate the data of a Rust source
/// code file and the `CanyonEntity` (if so), helping Canyon to know what source
/// file contains a `#[canyon_entity]` annotation and restricting it to just one
/// annotated struct per file.
///
/// This limitation it's imposed by desing. Canyon, when manages all the entities in
/// the user's source code, needs to know for future migrations the old data about a structure
/// and the new modified one.
///
/// For example, let's say that you have a:
/// ```
/// pub struct Person {
///    /* some fields */
/// }
/// ```
///
/// and you decided to modify it's Ident and change it to `Human`.
///
/// Canyon will take care about modifying the Database, and `ALTER TABLE` to edit the actual data for you,
/// but, if it's not able to get the data to know that the old one is `Person` and the new one it's `Human`.
/// it will simply drop the table (losing all your data) and creating a new table `Human`.
///
/// So, we decised to follow the next approach:
/// Every entity annotated with a `#[canyon_entity]` annotation will be related to only unique Rust source
/// code file. If we find more, Canyon will raise and error saying that it does not allows to having more than
/// one managed entity per source file.
///
/// Then, we will store the entities data in a special table only for Canyon, where we will create the relation
/// between the source file, the entity and it's fields and data.
///
/// So, if the user wants or needs to modify the data of it's entity, Canyon can secure that will perform the
/// correct operations because we can't "remember" how that entity was, and how it should be now, avoiding
/// potencially dangerous operations due to lack of knowing what entity relates with new data.
///
/// The `memory field` HashMap is made by the filepath as a key, and the struct's name as value
#[derive(Debug)]
pub struct  CanyonMemory {
    pub memory: HashMap<String, String>,
    pub renamed_entities: HashMap<String, String>,
}

// Makes this structure able to make queries to the database
impl Transaction<Self> for CanyonMemory {}

impl CanyonMemory {
    /// Queries the database to retrieve internal data about the structures
    /// tracked by `CanyonSQL`
    /// 
    /// TODO fetch schemas if structures have not default ones
    #[allow(clippy::nonminimal_bool)]
    pub async fn remember(datasource_name: &str) -> Self {     
        let database_type = get_database_type_from_datasource_name(datasource_name).await;

        // Creates the memory table if not exists
        Self::create_memory(datasource_name, &database_type).await;
        
        // Retrieve the last status data from the `canyon_memory` table
        // TODO hardcoded schema for SQLSERVER development
        let res = Self::query("SELECT * FROM canyon_memory", &[], datasource_name)
            .await
            .expect("Error querying Canyon Memory");
        let mem_results = res.as_canyon_rows();
        
        // Manually maps the results
        let mut db_rows = Vec::new();
        for row in mem_results.iter() {
            let db_row = CanyonMemoryRow {
                id: row.get::<i32>("id"),
                filepath: row.get::<&str>("filepath"),
                struct_name: row.get::<&str>("struct_name")
            };
            db_rows.push(db_row); 
        }
        println!("DB rows: {:?}", &db_rows);
        
        // Parses the source code files looking for the #[canyon_entity] annotated classes
        let mut mem = Self {
            memory: HashMap::new(),
            renamed_entities: HashMap::new(),
        };
        Self::find_canyon_entity_annotated_structs(&mut mem).await;
        println!("In memory entities: {:?}", &mem);

        // Insert into the memory table the new discovered entities
        // Care, insert the new ones, delete the olds
        // Also, updates the registry when the fields changes
        let mut values_to_insert = String::new();
        let mut updates = Vec::new();

        for (filepath, struct_name) in &mem.memory {
            // When the filepath and the struct hasn't been modified and are already on db
            let already_in_db = db_rows.iter().any(|el| {
                (el.filepath == *filepath && el.struct_name == *struct_name)
                    || ((el.filepath != *filepath && el.struct_name == *struct_name)
                        || (el.filepath == *filepath && el.struct_name != *struct_name))
            });
            if !already_in_db {
                values_to_insert.push_str(format!("('{filepath}', '{struct_name}'),").as_str());
            }
            // When the struct or the filepath it's already on db but one of the two has been modified
            let need_to_update = db_rows.iter().find(|el| {
                (el.filepath == *filepath || el.struct_name == *struct_name)
                    && !(el.filepath == *filepath && el.struct_name == *struct_name)
            });

            // updated means: the old one. The value to update
            if let Some(old) = need_to_update {
                updates.push(old.struct_name.clone());
                QUERIES_TO_EXECUTE.lock().unwrap().push(format!(
                    "UPDATE canyon_memory SET filepath = '{}', struct_name = '{}' \
                            WHERE id = {}",
                        filepath, struct_name, old.id
                ));
 
                // if the updated element is the struct name, whe add it to the table_rename Hashmap
                let rename_table = &old.struct_name != struct_name;

                if rename_table {
                    mem.renamed_entities.insert(
                        struct_name.to_lowercase(), // The new one
                        old.struct_name.to_lowercase(), // The old one
                    );
                } 
            }
        }

        if !values_to_insert.is_empty() {
            values_to_insert.pop();
            values_to_insert.push(';');

            QUERIES_TO_EXECUTE.lock().unwrap().push(format!(
                "INSERT INTO canyon_memory (filepath, struct_name) VALUES {}",
                values_to_insert
            ));
        }

        // Deletes the records when a table is dropped on the previous Canyon run
        let in_memory = mem.memory.values().collect::<Vec<&String>>();
        db_rows.into_iter().for_each(|db_row| {
            if !in_memory.contains(&&db_row.struct_name.to_string()) && !updates.contains(&&db_row.struct_name) {
                QUERIES_TO_EXECUTE.lock().unwrap().push(format!(
                    "DELETE FROM canyon_memory WHERE struct_name = '{}'",
                    db_row.struct_name
                ));
            }
        });

        mem
    }

    /// Parses the Rust source code files to find the one who contains Canyon entities
    /// ie -> annotated with `#{canyon_entity}`
    async fn find_canyon_entity_annotated_structs(&mut self) {
        for file in WalkDir::new("./src")
            .into_iter()
            .filter_map(|file| file.ok())
        {
            if file.metadata().unwrap().is_file()
                && file.path().display().to_string().ends_with(".rs")
            {
                // Opening the source code file
                let contents = fs::read_to_string(file.path())
                    .expect("Something went wrong reading the file");

                let mut canyon_entity_macro_counter = 0;
                let mut struct_name = String::new();
                for line in contents.split('\n') {
                    if !line.starts_with("//") && line.contains("struct") {
                        struct_name.push_str(
                            line.split_whitespace()
                                .collect::<Vec<&str>>()
                                .get(2)
                                .unwrap_or(&"FAILED"),
                        )
                    }
                    if line.contains("#[") // separated checks for possible different paths
                        && line.contains("canyon_entity")
                        && !line.starts_with("//")
                    {
                        canyon_entity_macro_counter += 1;
                    }
                }

                match canyon_entity_macro_counter {
                    0 => (),
                    1 => {
                        self.memory.insert(
                            file.path().display().to_string().replace("\\", "/"),
                            struct_name,
                        );
                    },
                    _ => panic!(
                        "Canyon does not support having multiple structs annotated
                        with `#[canyon::entity]` on the same file when the `#[canyon]`
                        macro it's present on the program"
                    ),
                }
            }
        }
    }

    /// Generates, if not exists the `canyon_memory` table
    async fn create_memory(datasource_name: &str, database_type: &DatabaseType) {        
        let query = if database_type == &DatabaseType::PostgreSql {
            constants::postgresql_queries::CANYON_MEMORY_TABLE
        } else { constants::mssql_queries::CANYON_MEMORY_TABLE };
        
        Self::query(query, &[], datasource_name)
            .await
            .expect("Error creating the 'canyon_memory' table");
    }
}

/// Represents a single row from the `canyon_memory` table
#[derive(Debug)]
struct CanyonMemoryRow<'a> {
    id: i32,
    filepath: &'a str,
    struct_name: &'a str,
}
