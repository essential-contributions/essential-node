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
    decl_const_sql_str!(CONTRACT_PREDICATE, "create/contract_predicate.sql");
    decl_const_sql_str!(CONTRACT_PROGRESS, "create/contract_progress.sql");
    decl_const_sql_str!(CONTRACT, "create/contract.sql");
    decl_const_sql_str!(DEC_VAR, "create/dec_var.sql");
    decl_const_sql_str!(DEPLOYED_CONTRACT, "create/deployed_contract.sql");
    decl_const_sql_str!(FAILED_BLOCK, "create/failed_block.sql");
    decl_const_sql_str!(FINALIZED_BLOCK, "create/finalized_block.sql");
    decl_const_sql_str!(MUTATION, "create/mutation.sql");
    decl_const_sql_str!(PREDICATE, "create/predicate.sql");
    decl_const_sql_str!(PUB_VAR, "create/pub_var.sql");
    decl_const_sql_str!(SOLUTION, "create/solution.sql");
    decl_const_sql_str!(STATE, "create/state.sql");
    decl_const_sql_str!(STATE_PROGRESS, "create/state_progress.sql");
    decl_const_sql_str!(VALIDATION_PROGRESS, "create/validation_progress.sql");
}

/// Statements for inserting rows into the tables.
pub mod insert {
    decl_const_sql_str!(BLOCK, "insert/block.sql");
    decl_const_sql_str!(BLOCK_SOLUTION, "insert/block_solution.sql");
    decl_const_sql_str!(CONTRACT, "insert/contract.sql");
    decl_const_sql_str!(CONTRACT_PREDICATE, "insert/contract_predicate.sql");
    decl_const_sql_str!(CONTRACT_PROGRESS, "insert/contract_progress.sql");
    decl_const_sql_str!(DEC_VAR, "insert/dec_var.sql");
    decl_const_sql_str!(DEPLOYED_CONTRACT, "insert/deployed_contract.sql");
    decl_const_sql_str!(FAILED_BLOCK, "insert/failed_block.sql");
    decl_const_sql_str!(FINALIZE_BLOCK, "insert/finalize_block.sql");
    decl_const_sql_str!(MUTATION, "insert/mutation.sql");
    decl_const_sql_str!(PREDICATE, "insert/predicate.sql");
    decl_const_sql_str!(PUB_VAR, "insert/pub_var.sql");
    decl_const_sql_str!(SOLUTION, "insert/solution.sql");
    decl_const_sql_str!(STATE_PROGRESS, "insert/state_progress.sql");
    decl_const_sql_str!(VALIDATION_PROGRESS, "insert/validation_progress.sql");
}

/// Statements for making queries.
pub mod query {
    decl_const_sql_str!(GET_CONTRACT_PREDICATES, "query/get_contract_predicates.sql");
    decl_const_sql_str!(GET_CONTRACT_PROGRESS, "query/get_contract_progress.sql");
    decl_const_sql_str!(GET_CONTRACT_SALT, "query/get_contract_salt.sql");
    decl_const_sql_str!(GET_BLOCK_NUMBER, "query/get_block_number.sql");
    decl_const_sql_str!(GET_LATEST_BLOCK_NUMBER, "query/get_latest_block_number.sql");
    decl_const_sql_str!(
        GET_LATEST_FINALIZED_BLOCK_ADDRESS,
        "query/get_latest_finalized_block_address.sql"
    );
    decl_const_sql_str!(GET_PREDICATE, "query/get_predicate.sql");
    decl_const_sql_str!(GET_SOLUTION, "query/get_solution.sql");
    decl_const_sql_str!(GET_STATE_PROGRESS, "query/get_state_progress.sql");
    decl_const_sql_str!(GET_STATE, "query/get_state.sql");
    decl_const_sql_str!(GET_VALIDATION_PROGRESS, "query/get_validation_progress.sql");
    decl_const_sql_str!(LIST_BLOCKS, "query/list_blocks.sql");
    decl_const_sql_str!(LIST_CONTRACTS, "query/list_contracts.sql");
    decl_const_sql_str!(LIST_BLOCKS_BY_TIME, "query/list_blocks_by_time.sql");
    decl_const_sql_str!(LIST_CONTRACTS_BY_TIME, "query/list_contracts_by_time.sql");
    decl_const_sql_str!(LIST_FAILED_BLOCKS, "query/list_failed_blocks.sql");
    decl_const_sql_str!(
        QUERY_STATE_AT_BLOCK_FINALIZED,
        "query/query_state_at_block_finalized.sql"
    );
    decl_const_sql_str!(
        QUERY_STATE_AT_SOLUTION_FINALIZED,
        "query/query_state_at_solution_finalized.sql"
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
    pub const CONTRACT: Table = Table::new("contract", create::CONTRACT);
    pub const CONTRACT_PREDICATE: Table =
        Table::new("contract_predicate", create::CONTRACT_PREDICATE);
    pub const CONTRACT_PROGRESS: Table = Table::new("contract_progress", create::CONTRACT_PROGRESS);
    pub const DEC_VAR: Table = Table::new("dec_var", create::DEC_VAR);
    pub const DEPLOYED_CONTRACT: Table = Table::new("deployed_contract", create::DEPLOYED_CONTRACT);
    pub const FAILED_BLOCK: Table = Table::new("failed_block", create::FAILED_BLOCK);
    pub const FINALIZED_BLOCK: Table = Table::new("finalized_block", create::FINALIZED_BLOCK);
    pub const MUTATION: Table = Table::new("mutation", create::MUTATION);
    pub const PREDICATE: Table = Table::new("predicate", create::PREDICATE);
    pub const PUB_VAR: Table = Table::new("pub_var", create::PUB_VAR);
    pub const SOLUTION: Table = Table::new("solution", create::SOLUTION);
    pub const STATE: Table = Table::new("state", create::STATE);
    pub const STATE_PROGRESS: Table = Table::new("state_progress", create::STATE_PROGRESS);
    pub const VALIDATION_PROGRESS: Table =
        Table::new("validation_progress", create::VALIDATION_PROGRESS);

    /// All tables in a list. Useful for initialisation and testing.
    pub const ALL: &[Table] = &[
        BLOCK,
        BLOCK_SOLUTION,
        CONTRACT,
        CONTRACT_PREDICATE,
        CONTRACT_PROGRESS,
        DEC_VAR,
        DEPLOYED_CONTRACT,
        FAILED_BLOCK,
        FINALIZED_BLOCK,
        MUTATION,
        PREDICATE,
        PUB_VAR,
        SOLUTION,
        STATE,
        STATE_PROGRESS,
        VALIDATION_PROGRESS,
    ];
}
