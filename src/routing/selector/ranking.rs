use super::{SmartSelector, WeightedAuth};
use crate::registry::{ModelInfo, PolicyContext};
use crate::routing::policy_weight::PolicyAwareWeightCalculator;
use crate::routing::weight::AuthInfo;
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

        Some(Self::select_by_weight(available))
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

        Some(Self::select_by_weight(available))
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
            .map_or(1.0, |m| m.calculate_weight_factor(model, context));

        let is_blocked = self
            .policy_matcher
            .as_ref()
            .is_some_and(|m| m.is_blocked(model, context));

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

            let weight = self
                .calculator
                .as_any()
                .downcast_ref::<PolicyAwareWeightCalculator>()
                .map_or_else(
                    || self.calculator.calculate(&auth, metrics.as_ref(), health) * policy_factor,
                    |policy_calc| {
                        let (_, _, final_weight) = policy_calc.calculate_with_policy(
                            &auth,
                            metrics.as_ref(),
                            health,
                            model,
                            context,
                        );
                        final_weight
                    },
                );

            if weight > 0.0 {
                available.push(WeightedAuth {
                    id: auth.id,
                    weight,
                });
            }
        }

        available
    }

    /// Select auth by weighted random choice.
    // ALLOW: Each expect is guarded by a prior length/index check that guarantees the element exists.
    #[allow(clippy::expect_used)]
    fn select_by_weight(available: Vec<WeightedAuth>) -> String {
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

        let r = rand::thread_rng().r#gen::<f64>() * total_weight;
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
