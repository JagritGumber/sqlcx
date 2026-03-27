# evolvsql

evolvsql is the adaptive engine vision behind sqlcx.

This document is intentionally conservative. It does not claim that evolvsql is finished today. It describes the direction clearly so sqlcx users do not confuse codegen with the future engine layer.

## One line

Evolvsql is an adaptive SQL engine direction for systems that should improve as schema, queries, and application behavior evolve.

## What evolvsql is

Evolvsql is about the part of the stack that can learn over time from real usage.

That can include things like:
- query pattern awareness
- schema evolution awareness
- generation that improves with project context
- better defaults and optimizations as the system sees more real workloads

The key idea is simple:
- traditional tooling is mostly static
- applications are not static
- the SQL layer should get better as the app grows

## What evolvsql is not

Evolvsql is not:
- a claim that sqlcx replaces your database today
- a claim that sqlcx is "better Postgres"
- a synonym for pgrx
- a promise that every adaptive idea is already implemented
- marketing fluff without a real systems boundary

## sqlcx vs evolvsql

The split should stay clear.

sqlcx:
- product you can use today
- SQL-first codegen
- cross-engine
- cross-language
- typed clients and schema output
- works with normal drivers and existing databases

evolvsql:
- deeper engine/runtime direction
- adaptive behavior over time
- optional vision layer, not a requirement for using sqlcx

Short version:
- sqlcx is the developer-facing product
- evolvsql is the adaptive engine direction

## Why keep them separate

Because mixing them creates confusion.

If we say sqlcx is the engine, people will expect:
- a new database
- a full runtime replacement
- strong claims that are not true yet

If we say evolvsql is just codegen, we lose the point of the adaptive vision.

So the split is useful:
- sqlcx ships practical value now
- evolvsql names the longer-term adaptive system

## Current honest positioning

Today, the safest honest message is:

- sqlcx gives you typed SQL codegen across languages and engines
- evolvsql is the adaptive direction for how the underlying SQL layer should improve over time

That keeps the story ambitious without pretending the future is already shipped.

## Messaging rules

Use language like:
- adaptive SQL engine direction
- adaptive SQL layer
- learning SQL system
- improves with schema, queries, and app context

Avoid language like:
- better Postgres
- smarter Postgres
- Postgres replacement
- self-learning database, if it is being used as a hard product claim today
- pgrx-led marketing

## Where this can go

If the engine vision becomes real, evolvsql can eventually stand for:
- adaptive query planning around app patterns
- smarter generation from observed usage
- tighter feedback loops between schema, queries, and generated clients
- a system that gets more useful as the application matures

But until that exists clearly, the copy should stay grounded.

## Recommended public framing right now

Use this split:

- sqlcx: typed SQL for every language, without forcing a new runtime
- evolvsql: the adaptive engine direction behind the system

That is clear, ambitious, and honest.
