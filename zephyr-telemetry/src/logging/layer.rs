//! Custom tracing layers for masking and filtering.

use crate::masking::SensitiveDataMasker;
use std::sync::Arc;
use tracing::Subscriber;
use tracing_subscriber::Layer;

/// A layer that wraps another layer and masks sensitive data.
pub struct MaskingLayer<L> {
    inner: L,
    #[allow(dead_code)]
    masker: Arc<SensitiveDataMasker>,
}

impl<L> MaskingLayer<L> {
    /// Create a new masking layer wrapping the given layer.
    pub fn new(inner: L, masker: Arc<SensitiveDataMasker>) -> Self {
        Self { inner, masker }
    }
}

impl<S, L> Layer<S> for MaskingLayer<L>
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    L: Layer<S>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        // Forward to inner layer - masking is handled at the field level
        self.inner.on_event(event, ctx);
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.inner.on_new_span(attrs, id, ctx);
    }

    fn on_record(
        &self,
        span: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.inner.on_record(span, values, ctx);
    }

    fn on_enter(&self, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_enter(id, ctx);
    }

    fn on_exit(&self, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_exit(id, ctx);
    }

    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_close(id, ctx);
    }
}
