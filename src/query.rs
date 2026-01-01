//! Filter-based search API for model queries.
//!
//! Provides a fluent builder pattern for filtering and searching models:
//!
//! ```ignore
//! let models = registry.query()
//!     .service("openai")
//!     .verified()
//!     .with_vision()
//!     .fuzzy("gpt turbo")
//!     .list();
//! ```

use regex::Regex;

use crate::client::ConfigProvider;
use crate::config::{ModelConfig, VerificationStatus};
use crate::registry_index::RegistryIndex;

/// A filter criterion for model queries.
#[derive(Debug, Clone)]
pub enum Filter {
    /// Exact service name match
    Service(String),
    /// Glob pattern match on service name (e.g., "open*")
    ServicePattern(String),
    /// Exact family name match
    Family(String),
    /// Glob pattern match on family name
    FamilyPattern(String),
    /// Verification status match
    Status(VerificationStatus),
    /// Capability check (name, required value)
    Capability(CapabilityFilter),
    /// Price range filter
    PriceRange {
        max_input: Option<f64>,
        max_output: Option<f64>,
    },
    /// Minimum context window
    ContextMin(u32),
    /// Maximum context window
    ContextMax(u32),
    /// Fuzzy match on model ID or name
    Fuzzy(String),
    /// Exact model ID match
    ModelId(String),
    /// Glob pattern match on model ID
    ModelIdPattern(String),
}

/// Capability filters.
#[derive(Debug, Clone)]
pub enum CapabilityFilter {
    Vision(bool),
    Streaming(bool),
    Functions(bool),
    JsonMode(bool),
    SystemPrompt(bool),
    Multimodal(bool),
}

/// Result ordering options.
#[derive(Debug, Clone, Copy, Default)]
pub enum OrderBy {
    /// No specific order (default)
    #[default]
    None,
    /// Order by input price (cheapest first)
    PriceAsc,
    /// Order by input price (most expensive first)
    PriceDesc,
    /// Order by context window (largest first)
    ContextDesc,
    /// Order by context window (smallest first)
    ContextAsc,
    /// Order by fuzzy match score (best first)
    FuzzyScore,
}

/// Fluent query builder for filtering models.
pub struct ModelQuery<'a, P: ConfigProvider + ?Sized> {
    provider: &'a P,
    index: &'a RegistryIndex,
    filters: Vec<Filter>,
    order_by: OrderBy,
    limit: Option<usize>,
}

impl<'a, P: ConfigProvider + ?Sized> ModelQuery<'a, P> {
    /// Create a new query builder.
    pub fn new(provider: &'a P, index: &'a RegistryIndex) -> Self {
        Self {
            provider,
            index,
            filters: Vec::new(),
            order_by: OrderBy::None,
            limit: None,
        }
    }

    // === Service Filters ===

    /// Filter by exact service name.
    pub fn service(mut self, name: impl Into<String>) -> Self {
        self.filters.push(Filter::Service(name.into()));
        self
    }

    /// Filter by service name pattern (glob-style: * and ?).
    pub fn service_like(mut self, pattern: impl Into<String>) -> Self {
        self.filters.push(Filter::ServicePattern(pattern.into()));
        self
    }

    // === Family Filters ===

    /// Filter by exact family name.
    pub fn family(mut self, name: impl Into<String>) -> Self {
        self.filters.push(Filter::Family(name.into()));
        self
    }

    /// Filter by family name pattern (glob-style).
    pub fn family_like(mut self, pattern: impl Into<String>) -> Self {
        self.filters.push(Filter::FamilyPattern(pattern.into()));
        self
    }

    // === Status Filters ===

    /// Filter by verification status.
    pub fn status(mut self, status: VerificationStatus) -> Self {
        self.filters.push(Filter::Status(status));
        self
    }

    /// Shorthand for status(Verified).
    pub fn verified(self) -> Self {
        self.status(VerificationStatus::Verified)
    }

    /// Shorthand for status(Unverified).
    pub fn unverified(self) -> Self {
        self.status(VerificationStatus::Unverified)
    }

    // === Capability Filters ===

    /// Filter by vision capability.
    pub fn with_vision(mut self) -> Self {
        self.filters
            .push(Filter::Capability(CapabilityFilter::Vision(true)));
        self
    }

    /// Filter by streaming capability.
    pub fn with_streaming(mut self) -> Self {
        self.filters
            .push(Filter::Capability(CapabilityFilter::Streaming(true)));
        self
    }

    /// Filter by function calling capability.
    pub fn with_functions(mut self) -> Self {
        self.filters
            .push(Filter::Capability(CapabilityFilter::Functions(true)));
        self
    }

    /// Filter by JSON mode capability.
    pub fn with_json_mode(mut self) -> Self {
        self.filters
            .push(Filter::Capability(CapabilityFilter::JsonMode(true)));
        self
    }

    /// Filter by system prompt support.
    pub fn with_system_prompt(mut self) -> Self {
        self.filters
            .push(Filter::Capability(CapabilityFilter::SystemPrompt(true)));
        self
    }

    /// Filter by multimodal capability.
    pub fn with_multimodal(mut self) -> Self {
        self.filters
            .push(Filter::Capability(CapabilityFilter::Multimodal(true)));
        self
    }

    // === Price Filters ===

    /// Filter by maximum price (input and output per 1k tokens).
    pub fn max_price(mut self, input: f64, output: f64) -> Self {
        self.filters.push(Filter::PriceRange {
            max_input: Some(input),
            max_output: Some(output),
        });
        self
    }

    /// Filter by maximum input price per 1k tokens.
    pub fn max_input_price(mut self, price: f64) -> Self {
        self.filters.push(Filter::PriceRange {
            max_input: Some(price),
            max_output: None,
        });
        self
    }

    /// Filter by maximum output price per 1k tokens.
    pub fn max_output_price(mut self, price: f64) -> Self {
        self.filters.push(Filter::PriceRange {
            max_input: None,
            max_output: Some(price),
        });
        self
    }

    // === Context Filters ===

    /// Filter by minimum context window size.
    pub fn context_min(mut self, tokens: u32) -> Self {
        self.filters.push(Filter::ContextMin(tokens));
        self
    }

    /// Filter by maximum context window size.
    pub fn context_max(mut self, tokens: u32) -> Self {
        self.filters.push(Filter::ContextMax(tokens));
        self
    }

    // === Search Filters ===

    /// Fuzzy search on model ID and name.
    pub fn fuzzy(mut self, query: impl Into<String>) -> Self {
        self.filters.push(Filter::Fuzzy(query.into()));
        self.order_by = OrderBy::FuzzyScore;
        self
    }

    /// Filter by exact model ID.
    pub fn model_id(mut self, id: impl Into<String>) -> Self {
        self.filters.push(Filter::ModelId(id.into()));
        self
    }

    /// Filter by model ID pattern (glob-style).
    pub fn model_id_like(mut self, pattern: impl Into<String>) -> Self {
        self.filters.push(Filter::ModelIdPattern(pattern.into()));
        self
    }

    // === Ordering ===

    /// Order results by price (cheapest first).
    pub fn order_by_price_asc(mut self) -> Self {
        self.order_by = OrderBy::PriceAsc;
        self
    }

    /// Order results by price (most expensive first).
    pub fn order_by_price_desc(mut self) -> Self {
        self.order_by = OrderBy::PriceDesc;
        self
    }

    /// Order results by context window (largest first).
    pub fn order_by_context_desc(mut self) -> Self {
        self.order_by = OrderBy::ContextDesc;
        self
    }

    /// Limit the number of results.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    // === Terminal Methods ===

    /// Execute the query and return matching model IDs.
    pub fn list(&self) -> Vec<&str> {
        let mut results = self.execute_filters();
        self.apply_ordering(&mut results);

        if let Some(limit) = self.limit {
            results.truncate(limit);
        }

        results.into_iter().map(|(id, _)| id).collect()
    }

    /// Execute the query and return the first matching model ID.
    pub fn first(&self) -> Option<&str> {
        self.list().into_iter().next()
    }

    /// Execute the query and return the count of matching models.
    pub fn count(&self) -> usize {
        self.execute_filters().len()
    }

    /// Execute the query and return the cheapest matching model.
    pub fn cheapest(&self) -> Option<&str> {
        let results = self.execute_filters();

        results
            .into_iter()
            .min_by(|(id_a, _), (id_b, _)| {
                let cost_a = self.get_model_cost(id_a);
                let cost_b = self.get_model_cost(id_b);
                cost_a
                    .partial_cmp(&cost_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| id)
    }

    /// Execute the query and return full model configs.
    pub fn configs(&self) -> Vec<&ModelConfig> {
        self.list()
            .into_iter()
            .filter_map(|id| self.provider.get_model(id).ok())
            .collect()
    }

    // === Internal Methods ===

    /// Execute all filters and return (model_id, fuzzy_score) tuples.
    fn execute_filters(&self) -> Vec<(&str, i64)> {
        let mut candidates: Vec<(&str, i64)> = self
            .index
            .all_models()
            .iter()
            .map(|id| (id.as_str(), 0i64))
            .collect();

        for filter in &self.filters {
            candidates = self.apply_filter(candidates, filter);
        }

        candidates
    }

    fn apply_filter<'b>(
        &self,
        candidates: Vec<(&'b str, i64)>,
        filter: &Filter,
    ) -> Vec<(&'b str, i64)> {
        match filter {
            Filter::Service(name) => candidates
                .into_iter()
                .filter(|(id, _)| self.index.service_for_model(id) == Some(name.as_str()))
                .collect(),

            Filter::ServicePattern(pattern) => {
                let regex = self.glob_to_regex(pattern);
                candidates
                    .into_iter()
                    .filter(|(id, _)| {
                        self.index
                            .service_for_model(id)
                            .map(|s| regex.is_match(s))
                            .unwrap_or(false)
                    })
                    .collect()
            }

            Filter::Family(name) => candidates
                .into_iter()
                .filter(|(id, _)| {
                    self.provider
                        .get_model(id)
                        .map(|m| m.model.family == *name)
                        .unwrap_or(false)
                })
                .collect(),

            Filter::FamilyPattern(pattern) => {
                let regex = self.glob_to_regex(pattern);
                candidates
                    .into_iter()
                    .filter(|(id, _)| {
                        self.provider
                            .get_model(id)
                            .map(|m| regex.is_match(&m.model.family))
                            .unwrap_or(false)
                    })
                    .collect()
            }

            Filter::Status(status) => candidates
                .into_iter()
                .filter(|(id, _)| {
                    self.provider
                        .get_model(id)
                        .map(|m| m.model.status == *status)
                        .unwrap_or(false)
                })
                .collect(),

            Filter::Capability(cap) => candidates
                .into_iter()
                .filter(|(id, _)| self.check_capability(id, cap))
                .collect(),

            Filter::PriceRange {
                max_input,
                max_output,
            } => candidates
                .into_iter()
                .filter(|(id, _)| {
                    if let Ok(m) = self.provider.get_model(id) {
                        let input_ok = max_input
                            .map(|max| m.pricing.input_per_1k_tokens <= max)
                            .unwrap_or(true);
                        let output_ok = max_output
                            .map(|max| m.pricing.output_per_1k_tokens <= max)
                            .unwrap_or(true);
                        input_ok && output_ok
                    } else {
                        false
                    }
                })
                .collect(),

            Filter::ContextMin(min) => candidates
                .into_iter()
                .filter(|(id, _)| {
                    self.provider
                        .get_model(id)
                        .map(|m| m.capabilities.context_window >= *min)
                        .unwrap_or(false)
                })
                .collect(),

            Filter::ContextMax(max) => candidates
                .into_iter()
                .filter(|(id, _)| {
                    self.provider
                        .get_model(id)
                        .map(|m| m.capabilities.context_window <= *max)
                        .unwrap_or(false)
                })
                .collect(),

            Filter::Fuzzy(query) => {
                // Cache lowercase query once for all candidates
                let query_lower = query.to_lowercase();
                // Pre-split query words once for reuse in fuzzy scoring
                let query_words: Vec<&str> = query_lower
                    .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
                    .filter(|s| !s.is_empty())
                    .collect();
                candidates
                    .into_iter()
                    .filter_map(|(id, _)| {
                        let score =
                            self.fuzzy_score_with_cached_query(id, &query_lower, &query_words);
                        if score > 0 {
                            Some((id, score))
                        } else {
                            None
                        }
                    })
                    .collect()
            }

            Filter::ModelId(target_id) => candidates
                .into_iter()
                .filter(|(id, _)| *id == target_id.as_str())
                .collect(),

            Filter::ModelIdPattern(pattern) => {
                let regex = self.glob_to_regex(pattern);
                candidates
                    .into_iter()
                    .filter(|(id, _)| regex.is_match(id))
                    .collect()
            }
        }
    }

    fn check_capability(&self, model_id: &str, cap: &CapabilityFilter) -> bool {
        let Ok(model) = self.provider.get_model(model_id) else {
            return false;
        };

        match cap {
            CapabilityFilter::Vision(v) => model.capabilities.vision == *v,
            CapabilityFilter::Streaming(v) => model.capabilities.streaming == *v,
            CapabilityFilter::Functions(v) => model.capabilities.functions == *v,
            CapabilityFilter::JsonMode(v) => model.capabilities.json_mode == *v,
            CapabilityFilter::SystemPrompt(v) => model.capabilities.system_prompt == *v,
            CapabilityFilter::Multimodal(v) => model.capabilities.multimodal == *v,
        }
    }

    fn apply_ordering(&self, results: &mut Vec<(&str, i64)>) {
        match self.order_by {
            OrderBy::None => {}
            OrderBy::PriceAsc => {
                results.sort_by(|(a, _), (b, _)| {
                    let cost_a = self.get_model_cost(a);
                    let cost_b = self.get_model_cost(b);
                    cost_a
                        .partial_cmp(&cost_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            OrderBy::PriceDesc => {
                results.sort_by(|(a, _), (b, _)| {
                    let cost_a = self.get_model_cost(a);
                    let cost_b = self.get_model_cost(b);
                    cost_b
                        .partial_cmp(&cost_a)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            OrderBy::ContextDesc => {
                results.sort_by(|(a, _), (b, _)| {
                    let ctx_a = self.get_context_window(a);
                    let ctx_b = self.get_context_window(b);
                    ctx_b.cmp(&ctx_a)
                });
            }
            OrderBy::ContextAsc => {
                results.sort_by(|(a, _), (b, _)| {
                    let ctx_a = self.get_context_window(a);
                    let ctx_b = self.get_context_window(b);
                    ctx_a.cmp(&ctx_b)
                });
            }
            OrderBy::FuzzyScore => {
                results.sort_by(|(_, score_a), (_, score_b)| score_b.cmp(score_a));
            }
        }
    }

    fn get_model_cost(&self, model_id: &str) -> f64 {
        self.provider
            .get_model(model_id)
            .map(|m| {
                // Weighted average: 70% input, 30% output
                (m.pricing.input_per_1k_tokens * 0.7) + (m.pricing.output_per_1k_tokens * 0.3)
            })
            .unwrap_or(f64::MAX)
    }

    fn get_context_window(&self, model_id: &str) -> u32 {
        self.provider
            .get_model(model_id)
            .map(|m| m.capabilities.context_window)
            .unwrap_or(0)
    }

    fn glob_to_regex(&self, pattern: &str) -> Regex {
        let escaped = regex::escape(pattern);
        let regex_pattern = escaped.replace(r"\*", ".*").replace(r"\?", ".");
        let regex_pattern = format!("^{}$", regex_pattern);
        Regex::new(&regex_pattern).unwrap_or_else(|_| Regex::new("^$").unwrap())
    }

    /// Optimized fuzzy matching with pre-cached lowercase query and query words.
    /// Returns higher scores for better matches.
    fn fuzzy_score_with_cached_query(
        &self,
        model_id: &str,
        query_lower: &str,
        query_words: &[&str],
    ) -> i64 {
        // Use eq_ignore_ascii_case for exact match checks to avoid allocation
        if model_id.eq_ignore_ascii_case(query_lower) {
            return 1000;
        }

        // Get model name for additional matching
        let model_name = self
            .provider
            .get_model(model_id)
            .map(|m| m.model.name.as_str())
            .unwrap_or("");

        if model_name.eq_ignore_ascii_case(query_lower) {
            return 900;
        }

        let mut score = 0i64;

        // For substring checks, we need lowercase versions
        // Cache these once per model instead of repeatedly
        let id_lower = model_id.to_lowercase();
        let name_lower = model_name.to_lowercase();

        // Contains full query
        if id_lower.contains(query_lower) {
            score += 500;
        }
        if name_lower.contains(query_lower) {
            score += 400;
        }

        // Starts with query
        if id_lower.starts_with(query_lower) {
            score += 200;
        }
        if name_lower.starts_with(query_lower) {
            score += 150;
        }

        // Check for word matches using pre-split query words
        let id_words: Vec<&str> = id_lower
            .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
            .filter(|s| !s.is_empty())
            .collect();

        for query_word in query_words {
            // Exact word match
            if id_words.contains(query_word) {
                score += 100;
            }
            // Word starts with
            for id_word in &id_words {
                if id_word.starts_with(query_word) {
                    score += 50;
                }
            }
            // Substring in any word
            for id_word in &id_words {
                if id_word.contains(query_word) {
                    score += 25;
                }
            }
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedded_config::EmbeddedConfigLoader;

    #[test]
    fn test_query_by_service() {
        let loader = EmbeddedConfigLoader::new().expect("Failed to load configs");
        let index = RegistryIndex::build(&loader);

        let query = ModelQuery::new(&loader, &index).service("anthropic");
        let models = query.list();

        assert!(!models.is_empty());
        for model in &models {
            assert!(model.contains("claude"), "Model {} should be Claude", model);
        }
    }

    #[test]
    fn test_query_verified() {
        let loader = EmbeddedConfigLoader::new().expect("Failed to load configs");
        let index = RegistryIndex::build(&loader);

        let query = ModelQuery::new(&loader, &index).verified();
        let models = query.list();

        for model_id in &models {
            let model = loader.get_model(model_id).unwrap();
            assert_eq!(model.model.status, VerificationStatus::Verified);
        }
    }

    #[test]
    fn test_query_fuzzy() {
        let loader = EmbeddedConfigLoader::new().expect("Failed to load configs");
        let index = RegistryIndex::build(&loader);

        let query = ModelQuery::new(&loader, &index).fuzzy("claude sonnet");
        let models = query.list();

        assert!(!models.is_empty());
        // First result should be a good match
        assert!(models[0].contains("sonnet") || models[0].contains("claude"));
    }

    #[test]
    fn test_query_chaining() {
        let loader = EmbeddedConfigLoader::new().expect("Failed to load configs");
        let index = RegistryIndex::build(&loader);

        let query = ModelQuery::new(&loader, &index)
            .service("anthropic")
            .with_vision()
            .context_min(100_000);
        let models = query.list();

        for model_id in &models {
            let model = loader.get_model(model_id).unwrap();
            assert_eq!(model.model.service, "anthropic");
            assert!(model.capabilities.vision);
            assert!(model.capabilities.context_window >= 100_000);
        }
    }

    #[test]
    fn test_query_cheapest() {
        let loader = EmbeddedConfigLoader::new().expect("Failed to load configs");
        let index = RegistryIndex::build(&loader);

        let query = ModelQuery::new(&loader, &index).service("anthropic");
        let cheapest = query.cheapest();

        assert!(cheapest.is_some());
        // Haiku should be cheapest among Claude models
        let cheapest = cheapest.unwrap();
        println!("Cheapest Anthropic model: {}", cheapest);
    }
}
