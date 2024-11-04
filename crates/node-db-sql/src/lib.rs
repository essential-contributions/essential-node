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
    decl_const_sql_str!(BLOCK_SOLUTION, "create/block_solution.sql");
    decl_const_sql_str!(DEC_VAR, "create/dec_var.sql");
    decl_const_sql_str!(FAILED_BLOCK, "create/failed_block.sql");
    decl_const_sql_str!(FINALIZED_BLOCK, "create/finalized_block.sql");
    decl_const_sql_str!(MUTATION, "create/mutation.sql");
    decl_const_sql_str!(SOLUTION, "create/solution.sql");
    decl_const_sql_str!(STATE, "create/state.sql");
    decl_const_sql_str!(VALIDATION_PROGRESS, "create/validation_progress.sql");
}

/// Statements for inserting rows into the tables.
pub mod insert {
    decl_const_sql_str!(BLOCK, "insert/block.sql");
    decl_const_sql_str!(BLOCK_SOLUTION, "insert/block_solution.sql");
    decl_const_sql_str!(DEC_VAR, "insert/dec_var.sql");
    decl_const_sql_str!(FAILED_BLOCK, "insert/failed_block.sql");
    decl_const_sql_str!(FINALIZE_BLOCK, "insert/finalize_block.sql");
    decl_const_sql_str!(MUTATION, "insert/mutation.sql");
    decl_const_sql_str!(SOLUTION, "insert/solution.sql");
    decl_const_sql_str!(VALIDATION_PROGRESS, "insert/validation_progress.sql");
}

/// Statements for making queries.
pub mod query {
    decl_const_sql_str!(GET_BLOCK_HEADER, "query/get_block_header.sql");
    decl_const_sql_str!(GET_BLOCK, "query/get_block.sql");
    decl_const_sql_str!(GET_LATEST_BLOCK_NUMBER, "query/get_latest_block_number.sql");
    decl_const_sql_str!(
        GET_LATEST_FINALIZED_BLOCK_ADDRESS,
        "query/get_latest_finalized_block_address.sql"
    );
    decl_const_sql_str!(
        GET_NEXT_BLOCK_ADDRESSES,
        "query/get_next_block_addresses.sql"
    );
    decl_const_sql_str!(
        GET_PARENT_BLOCK_ADDRESS,
        "query/get_parent_block_address.sql"
    );
    decl_const_sql_str!(GET_SOLUTION, "query/get_solution.sql");
    decl_const_sql_str!(GET_STATE, "query/get_state.sql");
    decl_const_sql_str!(GET_VALIDATION_PROGRESS, "query/get_validation_progress.sql");
    decl_const_sql_str!(LIST_BLOCKS, "query/list_blocks.sql");
    decl_const_sql_str!(LIST_BLOCKS_BY_TIME, "query/list_blocks_by_time.sql");
    decl_const_sql_str!(LIST_FAILED_BLOCKS, "query/list_failed_blocks.sql");
    decl_const_sql_str!(LIST_UNCHECKED_BLOCKS, "query/list_unchecked_blocks.sql");
    decl_const_sql_str!(
        QUERY_STATE_AT_BLOCK_FINALIZED,
        "query/query_state_at_block_finalized.sql"
    );
    decl_const_sql_str!(
        QUERY_STATE_AT_SOLUTION_FINALIZED,
        "query/query_state_at_solution_finalized.sql"
    );
    decl_const_sql_str!(
        QUERY_STATE_BLOCK_ADDRESS,
        "query/query_state_block_address.sql"
    );
}

/// Statements for updating and deleting state.
pub mod update {
    decl_const_sql_str!(STATE, "update/state.sql");
    decl_const_sql_str!(DELETE_STATE, "update/delete_state.sql");
}

pub mod table {
    use crate::create;

    /// A table's name along with its create statement.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
    pub struct Table {
        /// The name of the table as declared in the create statement.
        pub name: &'static str,
        /// The table's create statement.
        pub create: &'static str,
    }

    impl Table {
        const fn new(name: &'static str, create: &'static str) -> Self {
            Self { name, create }
        }
    }

    pub const BLOCK: Table = Table::new("block", create::BLOCK);
    pub const BLOCK_SOLUTION: Table = Table::new("block_solution", create::BLOCK_SOLUTION);
    pub const DEC_VAR: Table = Table::new("dec_var", create::DEC_VAR);
    pub const FAILED_BLOCK: Table = Table::new("failed_block", create::FAILED_BLOCK);
    pub const FINALIZED_BLOCK: Table = Table::new("finalized_block", create::FINALIZED_BLOCK);
    pub const MUTATION: Table = Table::new("mutation", create::MUTATION);
    pub const SOLUTION: Table = Table::new("solution", create::SOLUTION);
    pub const STATE: Table = Table::new("state", create::STATE);
    pub const VALIDATION_PROGRESS: Table =
        Table::new("validation_progress", create::VALIDATION_PROGRESS);

    /// All tables in a list. Useful for initialisation and testing.
    pub const ALL: &[Table] = &[
        BLOCK,
        DEC_VAR,
        FINALIZED_BLOCK,
        MUTATION,
        SOLUTION,
        BLOCK_SOLUTION,
        FAILED_BLOCK,
        STATE,
        VALIDATION_PROGRESS,
    ];
}
