# System Context: csv-managed

This document guides AI-assisted and human contributions for a high‑performance Rust command-line tool that manages very large CSV/TSV datasets (hundreds of GB+) for data engineering, data science, and ML workflows. It establishes coding, testing, performance, and release practices so generated or manual changes remain consistent, robust, and memory‑efficient.

## 1. The Users & Stakeholders

* **Primary User:** Data Engineer responsible for wrangling data, cleaning, and transforming large CSV/TSV datasets, creating data pipelines that are highly performant, flexible, and scalable.
* **Pipeline Architect:** Designs the broader data orchestration (Airflow, Dagster, Bash) where this tool serves as a reliable atomic unit of work.
* **Data Steward:** Concerned with schema validation, data quality rules, and ensuring data integrity across boundaries.
* **Developer:** Rust developer contributing to the codebase, ensuring high performance and memory efficiency.
* **DevOps Engineer:** Responsible for deploying and maintaining the application in production environments, ensuring scalability and reliability.
* **End User:** Data scientists and analysts who utilize the processed datasets for analysis and machine learning tasks.
* **QA Engineer:** Ensures the quality and reliability of the application through rigorous testing and validation.
* **Product Manager:** Oversees the development process, prioritizes features, and ensures alignment with user needs and business goals.
* **Technical Writer:** Creates and maintains documentation for users and developers, ensuring clarity and accessibility of information.
* **Security Engineer:** Ensures the application adheres to security best practices and compliance requirements.
* **Performance Engineer:** Focuses on optimizing the application's performance, particularly for handling large datasets efficiently.

## 2. The Technology Stack

* **Language:** Rust (Latest Stable / Edition 2024)
* **Build System:** Cargo
* **Core CSV Engine:** `csv` crate (streaming, compliant)
* **CLI Framework:** `clap` (v4) for robust CLI argument parsing.
* **Serialization:** `serde`, `serde_json`, `serde_yaml_ng` (YAML/JSON schemas).
* **Data Types:** `rust_decimal` (financial precision), `chrono` (temporal), `uuid`.
* **Expression Engine:** `evalexpr` (dynamic filtering/calculation).
* **Error Handling:** `thiserror` (lib), `anyhow` (app), `log`/`env_logger`.
* **Benchmarking:** `criterion` for performance benchmarks.
* **Testing Framework:** built-in Rust test framework with `cargo test`.
* **CI/CD:** GitHub Actions for automated testing and deployment.
* **Packaging:** Crates.io for Rust package distribution.
* **Documentation:** mdBook for user guides and API documentation.
* **Performance Profiling:** cargo-flamegraph for profiling and optimizing performance.
* **Version Control:** Git for source code management.
* **Containerization:** Docker for containerizing the application for consistent deployment environments.
* **Cloud Provider:** AWS/GCP/Azure for hosting and scaling the application as needed.
* **Security Tools:** Dependabot for automated dependency updates and vulnerability scanning.
* **Code Quality Tools:** Clippy for Rust code linting and formatting.
* **Documentation Hosting:** GitHub Pages or Read the Docs for hosting user and developer documentation.
* **Collaboration Tools:** GitHub for issue tracking, pull requests, and code reviews.
* **Project Management:** GitHub for issues for managing tasks and project workflows.

## 3. Data Engineering Principles

* **Streaming First:** Operations must not load the entire file into RAM. Processing should happen row-by-row or in small chunks to support datasets larger than available memory.
* **Unix Philosophy:** The tool should function well in a pipeline, supporting `stdin` and `stdout` for piping (e.g., `cat data.csv | csv-managed ...`).
* **Schema-on-Read/Write:** Strict enforcement of data types (Dates, Decimals, Integers) is critical to prevent downstream failures in databases or ML models.
* **Zero-Copy:** Prefer borrowing (`&str`) over allocation (`String`) wherever possible to maximize throughput and minimize GC-like pauses.
* **Deterministic Output:** Running the same command on the same data must produce the exact same byte-for-byte output (sorting, hashing) to ensure reproducibility.

## 4. Global Constraints

* No new external dependencies without approval.
* All code must pass `cargo clippy --all-targets -D warnings` and be formatted with `cargo fmt`.
* Performance benchmarks must be included for any feature affecting data processing speed.
* All changes must be documented in the changelog.
* Code must be compatible with the latest stable Rust toolchain.
* Memory usage must be optimized for handling very large datasets (hundreds of GB+).
* All contributions must include unit tests with at least 90% code coverage.
* Integration tests must be provided for any new features or significant changes.
* All code must adhere to the project's coding standards and style guidelines.
* Security best practices must be followed, especially when handling sensitive data.
* No silent failures. Data rows that fail parsing or validation must be explicitly handled (logged, rejected, or aborted) based on configuration.
* Deterministic output is required.
* All changes must be reviewed and approved by at least one other developer before merging.
* Documentation must be updated to reflect any changes or new features.
* All CI/CD pipelines must pass successfully before any code is merged into the main branch.
* Performance regressions must be identified and addressed before release.
* All releases must follow semantic versioning principles.
* User feedback must be considered for future improvements and feature prioritization.
* All contributions must respect the project's license and intellectual property rights.
* Accessibility considerations must be taken into account for any user-facing features.
* All changes must be compatible with the existing architecture and design patterns of the codebase.
* Any breaking changes must be clearly communicated in the release notes.
* All contributions must be made in accordance with the project's contribution guidelines.
* Regular code reviews must be conducted to maintain code quality and consistency.
* All dependencies must be kept up to date to ensure security and performance.
* Any new features must be designed with scalability in mind to accommodate future growth.
* All changes must be tested in a staging environment before deployment to production.
* Performance optimizations must be documented and justified in the code comments.
* All contributions must be made with consideration for the overall user experience and usability of the tool.
* All changes must be compatible with the target operating systems (Linux, macOS, Windows).
* Any new features must be designed to minimize disruption to existing users and workflows.
* All contributions must be made with respect for the project's community and collaborative spirit.
* All changes must be reversible, with clear rollback procedures in place for production deployments.
* All contributions must be made with consideration for the long-term maintainability of the codebase.
* All changes must be made with an emphasis on code readability and clarity for future contributors.
* All contributions must be made with respect for the project's goals and vision for high-performance CSV/TSV data management.
* All changes must be made with consideration for the environmental impact of data processing and resource usage.
* All contributions must be made with an understanding of the competitive landscape and market needs for data management tools.
* All changes must be made with a focus on innovation and continuous improvement of the tool's capabilities.
* All contributions must be made with a commitment to open-source principles and community engagement.
* All changes must be made with an emphasis on collaboration and knowledge sharing among the development team.
* All contributions must be made with a focus on delivering value to the end users and stakeholders of the tool.
* All changes must be made with consideration for the evolving landscape of data engineering and data science practices
* All contributions must be made with a commitment to ethical data handling and privacy considerations.
* All changes must be made with an understanding of the technical debt and legacy code within the codebase.
* All contributions must be made with a focus on fostering a positive and inclusive community around the project
* All changes must be made with an emphasis on testing and validation to ensure reliability and robustness.
* All contributions must be made with consideration for the documentation and educational resources available to users and developers.
* All changes must be made with a commitment to transparency and accountability in the development process.
* All contributions must be made with a focus on aligning with the strategic goals and objectives of the project.
* All changes must be made with an understanding of the project's roadmap and future development plans.
* All contributions must be made with a commitment to continuous learning and professional development for the development team.
* All changes must be made with consideration for the feedback and suggestions from the user community.
* All contributions must be made with a focus on ensuring the scalability and adaptability of the tool to meet future data management challenges.
* All changes must be made with an emphasis on fostering a culture of innovation and creativity within the development team.
* All contributions must be made with a commitment to maintaining the integrity and reliability of the tool for its users.
* All changes must be made with consideration for the long-term sustainability and viability of the project in the open-source ecosystem.
* All contributions must be made with a focus on enhancing the overall user experience and satisfaction with the tool.
* All changes must be made with an understanding of the broader context of data management and the evolving needs of data engineers and data scientists.
* All contributions must be made with a commitment to fostering collaboration and partnerships with other open-source projects and communities.
* All changes must be made with an emphasis on ensuring the tool remains competitive and relevant in the rapidly changing landscape of data management technologies.
* All contributions must be made with a focus on delivering high-quality, reliable, and efficient solutions for managing very large CSV/TSV datasets.
* All changes must be made with consideration for the ethical implications of data management and the responsibilities of handling large datasets.
* All contributions must be made with a commitment to fostering a supportive and inclusive environment for all contributors to the project.
* **Release Manager:** Oversees the release process, ensuring that new versions are delivered smoothly and on schedule.
