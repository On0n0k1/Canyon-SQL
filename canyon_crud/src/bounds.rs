use tokio_postgres::types::{ToSql, FromSql};

/// Created for retrieve the field's name of a field of a struct, giving 
/// the Canoyn's autogenerated enum with the variants that maps this
/// fields.
/// 
/// ```
/// pub struct Struct<'a> {
///     pub some_field: &'a str
/// }
/// 
/// // Autogenerated enum
/// #[derive(Debug)]
/// #[allow(non_camel_case_types)]
/// pub enum StructField {
///     some_field
/// }
/// ```
/// So, to retrieve the field's name, something like this w'd be used on some part
/// of the Canyon's Manager crate, to wire the necessary code to pass the field
/// name, retrieved from the enum variant, to a called.
/// 
/// // Something like:
/// `let struct_field_name_from_variant = StructField::some_field.field_name_as_str();`
pub trait FieldIdentifier {
    fn field_name_as_str(self) -> String;
}

/// Represents some kind of introspection to make the implementors
/// retrieves a value inside some variant of an associated enum type.
/// and convert it to an [`String`], to enable the convertion of 
/// that value into something that can be part of an SQL query.
/// 
/// It's a generification to convert everything to a string representation
/// in SQL syntax, so the clauses can use any value to make filters
/// 
/// Ex:
/// `SELECT * FROM some_table WHERE id = '2'`
/// 
/// That '2' it's extracted from some enum that implements [`FieldValueIdentifier`],
/// where usually the variant w'd be something like:
/// 
/// ```
/// pub enum Enum {
///     IntVariant(i32)
/// }
/// ```
/// so, the `.value(self)` method it's called over `self`, gets the value for that variant
/// (or another specified in the logic) and returns that value as an [`String`]
pub trait FieldValueIdentifier {
    fn value(self) -> String;
}

impl FieldValueIdentifier for &str {
    fn value(self) -> String {
        self.to_string()
    }
}

/// Bounds to some type T in order to make it callable over some fn parameter T
/// 
/// Represents the ability of an struct to be considered as candidate to perform
/// actions over it as it holds the 'parent' side of a foreign key relation.
/// 
/// Usually, it's used on the Canyon macros to retrieve the column that 
/// this side of the relation it's representing
pub trait ForeignKeyable {
    /// Retrieves the field related to the column passed in
    fn get_fk_column(&self, column: &str) -> Option<String>;
}


/// To define trait objects that helps to relates the necessary bounds in the 'IN` SQL clause
pub trait InClauseValues: ToSql + ToString {}

/// Defines a trait to join types that can represent
/// PrimaryKey type.
/// 
/// Canyon only accepts values of i32, i64 
/// + any Rust String type that can work 
/// as any Rust string variation.
pub trait PrimaryKey: ToSql + Sync + Send {}
impl PrimaryKey for i32 {}
impl PrimaryKey for i64 {}
impl PrimaryKey for &str {}
impl PrimaryKey for String {}
impl PrimaryKey for &String {}