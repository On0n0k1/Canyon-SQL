# CANYON-SQL
A full written in `Rust` ORM

## Early stages
The library it's still on a `newborn` state. Any contrib via `fork` + `PR` it's really apreciated.

Availiable operations:
    - find all
    - find by id

## Basic example of usage

Assuming that the main goal of an `ORM` it's perform `SQL queries` based on certain object
oriented code, in order to map some data-model as an SQL entity...

1 - Due to the async nature of the library, we need to mark the implementation of the 
`CrudOperations` trait as `#[async_trait]`

2 - Implement the new or the empty constructors. You can also impl the `Default` trait for the standard library if you prefer.

3 - Implement the `RowMapper` trait. Map all of your struct attributes double ckeching the types on Rust, and what you need to get from database.

`my_model.rs` file
```
use canyon_sql::async_trait::*;
use canyon_sql::tokio;

#[derive(Debug)]
pub struct Foo {
    field: String
}

#[async_trait]
impl canyon_sql::crud::CrudOperations<Foo> for Foo { }


impl Foo {
    pub fn new(field_data: &str) -> Self {
        Self {
            field: field_data.to_string()
        }
    }

    pub fn empty() -> Self {
        Self {
            field: "".to_string()
        }
    }
}

impl RowMapper<Self> for Foo {
    fn deserialize(row: &Row) -> Self {
        Self {
            field: row.try_get("field")
                .expect("Failed to retrieve the FIELD field"),
        }
    }
}
```

And now, on your main file, just instanciate a new object of your custom type Foo.
You will have availiable (thanks to the `CrudOperations` trait) any option designed
as a member of your type.

NOTE: Remember to `await` the result of any trait's method. 
NOTE: For human-readable result, use the `.as_response::<Foo>()` method.


`main.rs` file
```
#[tokio::main]
async fn main() {

    let foo = Foo::empty();
    
    let all_foo = foo
        .find_all("foo", &[])
        .await
        .as_response::<Foo>();

    println!("All foo results from database: {:?}", all_foo);
}
```