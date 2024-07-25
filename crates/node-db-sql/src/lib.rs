//! Provides the SQL statements used by `essential-node-db` via `const` `str`s.

/// Short-hand for including an SQL string from the `sql/` subdir at compile time.
macro_rules! include_sql_str {
    ($subpath:expr) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/sql/", $subpath))
    };
}

/// Short-hand for declaring a `const` SQL str and presenting the SQL via the doc comment.
macro_rules! decl_const_sql_str {
    ($name:ident, $subpath:expr) => {
        /// ```sql
        #[doc = include_sql_str!($subpath)]
        /// ```
        pub const $name: &str = include_sql_str!($subpath);
    };
}

/// Table creation statements.
pub mod create {
    decl_const_sql_str!(BLOCK, "create/block.sql");
    decl_const_sql_str!(CONTRACT_PAIRING, "create/contract_pairing.sql");
    decl_const_sql_str!(CONTRACTS, "create/contracts.sql");
    decl_const_sql_str!(PREDICATES, "create/predicates.sql");
    decl_const_sql_str!(SOLUTIONS, "create/solutions.sql");
    decl_const_sql_str!(STATE, "create/state.sql");
}

/// Statements for inserting rows into the tables.
pub mod insert {
    decl_const_sql_str!(BLOCK, "insert/block.sql");
    decl_const_sql_str!(CONTRACTS, "insert/contracts.sql");
    decl_const_sql_str!(CONTRACT_PAIRING, "insert/contract_pairing.sql");
    decl_const_sql_str!(PREDICATES, "insert/predicates.sql");
    decl_const_sql_str!(SOLUTIONS, "insert/solutions.sql");
}

/// Statements for making queries.
pub mod query {
    decl_const_sql_str!(GET_CONTRACT, "query/get_contract.sql");
    decl_const_sql_str!(GET_PREDICATE, "query/get_predicate.sql");
    decl_const_sql_str!(GET_SOLUTION, "query/get_solution.sql");
    decl_const_sql_str!(GET_STATE, "query/get_state.sql");
    decl_const_sql_str!(LIST_BLOCKS, "query/list_blocks.sql");
    decl_const_sql_str!(LIST_CONTRACTS, "query/list_contracts.sql");
    decl_const_sql_str!(LIST_BLOCKS_BY_TIME, "query/list_blocks_by_time.sql");
    decl_const_sql_str!(LIST_CONTRACTS_BY_TIME, "query/list_contracts_by_time.sql");
}

/// Statements for updating and deleting state.
pub mod update {
    decl_const_sql_str!(STATE, "update/state.sql");
    decl_const_sql_str!(DELETE_STATE, "update/delete_state.sql");
}
