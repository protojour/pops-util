use std::{env, ffi::OsStr, time::Duration};

use http::Uri;
use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::{OTEL_EXPORTER_OTLP_ENDPOINT, OTEL_EXPORTER_OTLP_METRICS_ENDPOINT};
use opentelemetry_sdk::{
    Resource, metrics::SdkMeterProvider, propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use rand::seq::IndexedRandom;
use tracing::{Level, info, level_filters::LevelFilter};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    init();

    let mut rng = rand::rng();
    let messages = ["foo", "bar", "baz", "qux"];

    let counter = global::meter("testmeter")
        .u64_counter("testcounter")
        .with_description("description of stuff")
        .with_unit("bytes")
        .build();

    loop {
        let msg = messages.choose(&mut rng).unwrap();
        info!(msg, "doing the loop");

        counter.add(
            42,
            &[KeyValue::new("key", "value"), KeyValue::new("msg", *msg)],
        );

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

fn init() {
    // tracing
    let tracing_layer = tracing_subscriber::registry()
        .with(LevelFilter::from(Level::DEBUG))
        .with(tracing_subscriber::fmt::layer().with_target(true));

    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let otel_resource = Resource::builder()
        .with_service_name("otel-demo-service")
        .build();

    if let Ok(_uri) = uri_env(OTEL_EXPORTER_OTLP_ENDPOINT) {
        let trace_provider = SdkTracerProvider::builder()
            .with_resource(otel_resource.clone())
            .with_batch_exporter(
                opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .build()
                    .unwrap(),
            )
            .build();

        // do we have to do global:: and `tracing_layer.with`?
        opentelemetry::global::set_tracer_provider(trace_provider.clone());

        let tracer = trace_provider.tracer("tracing-otel-subscriber");

        tracing_layer.with(OpenTelemetryLayer::new(tracer)).init();
    } else {
        tracing_layer.init();
    }

    if let Ok(uri) = uri_env(OTEL_EXPORTER_OTLP_METRICS_ENDPOINT) {
        let meter_provider = SdkMeterProvider::builder()
            .with_resource(otel_resource)
            .with_periodic_exporter({
                let builder = opentelemetry_otlp::MetricExporter::builder();
                if uri.port_u16() == Some(4317) {
                    builder.with_tonic().build().unwrap()
                } else {
                    builder.with_http().build().unwrap()
                }
            })
            .build();

        opentelemetry::global::set_meter_provider(meter_provider);
    }
}

fn uri_env(key: impl AsRef<OsStr>) -> Result<Uri, anyhow::Error> {
    Ok(env::var(key)?.parse()?)
}
