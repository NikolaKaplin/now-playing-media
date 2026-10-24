mod validation;

#[cfg(not(test))]
mod addon;

#[cfg(test)]
#[path = "tests/lib.rs"]
mod tests;
