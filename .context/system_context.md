# System Context: csv-managed

This document guides AI-assisted and human contributions for a high‑performance Rust command-line tool that manages very large CSV/TSV datasets (hundreds of GB+) for data engineering, data science, and ML workflows. It establishes coding, testing, performance, and release practices so generated or manual changes remain consistent, robust, and memory‑efficient.

## 1. The Users & Stakeholders
* **Primary User:** [e.g., Senior Data Analyst] who values precision over speed.
* **Stakeholder:** [e.g., Security Team] requires all PII to be encrypted at rest.
* **Stakeholder:** [e.g., Marketing] requires all UI components to match the Design System.

## 2. The Technology Stack
* **Language:** TypeScript 5.0+
* **Framework:** React 18 (Next.js App Router)
* **Database:** PostgreSQL via Supabase

## 3. Global Constraints
* No new external dependencies without approval.
* All code must pass strict linting rules defined in `.eslintrc`.