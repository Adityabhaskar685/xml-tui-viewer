pub mod fuzzy;
pub mod regex_search;
pub mod xpath;

pub use fuzzy::fuzzy_search;
pub use regex_search::search as regex_search;
pub use xpath::xpath_search;
