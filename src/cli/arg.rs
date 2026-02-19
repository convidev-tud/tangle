use clap::ArgMatches;

#[derive(Debug)]
pub struct ArgHelper<'a> {
    args: &'a ArgMatches,
}
impl<'a> ArgHelper<'a> {
    pub fn new(matches: &'a ArgMatches) -> Self {
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
        self.args.contains_id(id)
    }
}
