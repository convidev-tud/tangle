use clap::ArgMatches;

#[derive(Debug, Clone)]
pub struct ArgHelper {
    args: ArgMatches,
}
impl ArgHelper {
    pub fn new(matches: ArgMatches) -> Self {
        Self { args: matches }
    }
    pub fn get_matches(&self) -> &ArgMatches {
        &self.args
    }
    pub fn get_argument_value<T: Clone + Send + Sync + 'static>(&self, id: &str) -> Option<T> {
        Some(self.args.get_one::<T>(id)?.clone())
    }
    pub fn get_argument_values<T: Clone + Send + Sync + 'static>(
        &self,
        id: &str,
    ) -> Option<Vec<T>> {
        Some(
            self.args
                .get_many::<T>(id)?
                .map(|s| s.clone())
                .collect::<Vec<_>>(),
        )
    }
    pub fn get_count(&self, id: &str) -> usize {
        self.args.get_count(id) as usize
    }
    pub fn has_arg(&self, id: &str) -> bool {
        self.args.try_contains_id(id).unwrap_or_else(|_| false)
    }
}
