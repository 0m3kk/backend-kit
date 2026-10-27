# application

Generic application-layer primitives shared by backend services.

Currently provides [`ExecutionContext`](src/context.rs), which carries request
metadata (actor, correlation, tracing, client details) and converts domain
events into `event-store` events enriched with that metadata.
