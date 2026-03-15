use super::{SmartSelector, WeightedAuth};
use crate::policy_weight::PolicyAwareWeightCalculator;
use crate::weight::AuthInfo;
use model_registry::{ModelInfo, PolicyContext};
use rand::Rng;

impl SmartSelector {
    /// Pick the best auth based on weighted selection
    pub async fn pick(&self, auths: Vec<AuthInfo>) -> Option<String> {
        if !self.config.enabled {
            return auths.into_iter().next().map(|a| a.id);
        }

        if auths.is_empty() {
            return None;
        }

        let available = self.filter_and_weigh(auths).await;

        if available.is_empty() {
            return None;
        }

        Some(self.select_by_weight(available))
    }

    /// Pick the best auth with policy-aware selection
    ///
    /// This method evaluates routing policies against the model and context,
    /// then adjusts weights accordingly.
    pub async fn pick_with_policy(
        &self,
        auths: Vec<AuthInfo>,
        model: &ModelInfo,
        context: &PolicyContext,
    ) -> Option<String> {
        if !self.config.enabled {
            return auths.into_iter().next().map(|a| a.id);
        }

        if auths.is_empty() {
            return None;
        }

        let available = self
            .filter_and_weigh_with_policy(auths, model, context)
            .await;

        if available.is_empty() {
            return None;
        }

        Some(self.select_by_weight(available))
    }

    /// Filter available auths and calculate weights (without policy)
    async fn filter_and_weigh(&self, auths: Vec<AuthInfo>) -> Vec<WeightedAuth> {
        let mut available = Vec::new();

        for auth in auths {
            if auth.unavailable {
                continue;
            }

            let metrics = self.metrics.get_metrics(&auth.id).await;
            let health = self.health.get_status(&auth.id).await;
            let is_available = self.health.is_available(&auth.id).await;

            if !is_available {
                continue;
            }

            let weight = self.calculator.calculate(&auth, metrics.as_ref(), health);

            if weight > 0.0 {
                available.push(WeightedAuth {
                    id: auth.id,
                    weight,
                });
            }
        }

        available
    }

    /// Filter available auths and calculate policy-aware weights
    async fn filter_and_weigh_with_policy(
        &self,
        auths: Vec<AuthInfo>,
        model: &ModelInfo,
        context: &PolicyContext,
    ) -> Vec<WeightedAuth> {
        let mut available = Vec::new();

        let policy_factor = self
            .policy_matcher
            .as_ref()
            .map(|m| m.calculate_weight_factor(model, context))
            .unwrap_or(1.0);

        let is_blocked = self
            .policy_matcher
            .as_ref()
            .map(|m| m.is_blocked(model, context))
            .unwrap_or(false);

        if is_blocked {
            return Vec::new();
        }

        for auth in auths {
            if auth.unavailable {
                continue;
            }

            let metrics = self.metrics.get_metrics(&auth.id).await;
            let health = self.health.get_status(&auth.id).await;
            let is_available = self.health.is_available(&auth.id).await;

            if !is_available {
                continue;
            }

            let weight = if let Some(policy_calc) = self
                .calculator
                .as_any()
                .downcast_ref::<PolicyAwareWeightCalculator>()
            {
                let (_, _, final_weight) = policy_calc.calculate_with_policy(
                    &auth,
                    metrics.as_ref(),
                    health,
                    model,
                    context,
                );
                final_weight
            } else {
                let base_weight = self.calculator.calculate(&auth, metrics.as_ref(), health);
                base_weight * policy_factor
            };

            if weight > 0.0 {
                available.push(WeightedAuth {
                    id: auth.id,
                    weight,
                });
            }
        }

        available
    }

    /// Select auth by weighted random choice
    fn select_by_weight(&self, available: Vec<WeightedAuth>) -> String {
        if available.len() == 1 {
            return available
                .into_iter()
                .next()
                .expect("should have element")
                .id;
        }

        let total_weight: f64 = available.iter().map(|a| a.weight).sum();

        if total_weight <= 0.0 {
            let idx = rand::thread_rng().gen_range(0..available.len());
            return available
                .into_iter()
                .nth(idx)
                .expect("should have element")
                .id;
        }

        let fallback = available
            .last()
            .map(|a| a.id.clone())
            .expect("should have element");

        let r = rand::thread_rng().gen::<f64>() * total_weight;
        let mut cumulative = 0.0;

        for auth in available {
            cumulative += auth.weight;
            if r <= cumulative {
                return auth.id;
            }
        }

        // SAFETY: Mathematically this loop should always match because:
        // 1. total_weight > 0 (checked above)
        // 2. r is in [0, total_weight)
        // 3. cumulative accumulates to total_weight
        // However, floating-point edge cases could theoretically miss,
        // so return the saved fallback.
        fallback
    }
}
