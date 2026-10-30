# Tickets

## Purpose and audience
A single-user Linear-style tracker for Calum and his agents to manage projects,
tickets, labels and dependencies. Humans work through the web UI; agents also
use the authenticated API. Prioritize quick scanning and efficient task editing.

## Platform
web

## Stack and constraints
Rust, Leptos CSR, Cloudflare Workers and D1 are locked. Preserve existing app
behavior and the shared shell. Tickets is dark only, regardless of OS or stored
theme preferences. Visual authority lives in DESIGN.md.
