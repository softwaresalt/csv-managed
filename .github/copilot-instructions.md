# Instructions

## Overview

The objective is to use the Rust programming language to create a super fast and efficient command-line tool to manage CSV files of all sizes.  
The primary user personas for this tool are data engineers, machine learning (ML) engineers, and data scientists.
Use cases:
- Preparing data for use in training machine learning models.
- Preparing data for use in data science experiments
- Wrangling data for a wide array of situations in data engineering and machine learning workflows.
- Probing and verifying data files for correctness before use in data pipelines.
- The tool should be capable of handling very large CSV files (100s of GBs or more) efficiently, with minimal memory usage and high performance. The tool should provide a variety of features to manipulate, validate, and transform CSV data, making it a versatile utility for data professionals.
- Flagging when data does not conform to expected schema definitions.
- Generating reports on data quality and integrity based on schema definitions.
- Transforming data to match specified schema requirements.
- Generating schema definitions from existing CSV files.


Features of this tool include:
- The ability to specify in the -schema.yml file a mapping of existing column names to new column names to be used in all outputs from the file.
- The ability to point the app at all files of the same file extension in a directory and verify each file against a -schema.yml file schema definition including data type verification.
- The ability to output the schema definition for a CSV file in a human-readable list format to the console output.
- The ability to index all of the files in a directory matching a single schema file.
- The ability to perform a union of multiple files that is able to deduplicate rows across multiple files and output to a single file.
- The ability to union all of the files in a directory in a sorted order and split into multiple files based on either row count per file or file size.
- The ability to consume a batch processing definition file in which all possible command-line arguments can be defined; file should be in JSON format.
- The ability to define a primary key (single column or composite) that uniquely identifies a row in the file. 
- The ability to add a fast hash signature for each row in a file to an index for the file that is defined as a primary key index. 
- The ability to probe a file for candidate primary key or composite key and print to console window candidate key(s)
- The ability to handle decimal data types of defined scope and precision
- The ability to transform fields from one data type to another; for example, a string to date, time, or datetime.
- The ability to define a currency datatype that restricts decimal precision to 4 digits to the right of the decimal point, probing for valid currency formats and ensuring correct parsing and validation.  Also the ability to transform a longer decimal or float value to a currency format for data standardization.
- Enhance data verification capabilities so that it can be used as an effective cloud implemented data validation tool for customers.

## Architecture Patterns

- Command-line utility
- Leverage standard library capabilities where possible for portability
- Write modular code with clear separation of concerns
- Use efficient data structures for handling large CSV files
- Implement error handling for robustness
- Use external crates for CSV parsing, B-Tree indexing, and command-line argument parsing
- Write unit tests and integration tests to ensure correctness
- Optimize for performance, especially for large files
- Provide clear documentation and usage instructions
- Follow Rust best practices for code style and organization
- Use Cargo for dependency management and building the project
- Implement logging for debugging and monitoring
- Consider memory usage and performance trade-offs when designing features
- Use iterators and lazy evaluation where possible to handle large datasets efficiently
- Design the tool to be extensible for future features and enhancements
- Ensure cross-platform compatibility for different operating systems
- Provide examples and sample CSV files for users to test the tool
- Use feature flags to enable or disable specific functionalities
- Use profiling tools to identify and optimize performance bottlenecks
- Ensure proper handling of edge cases, such as empty files, malformed CSVs, and large datasets
- Provide a user-friendly command-line interface with clear help messages and documentation
- Use serialization and deserialization libraries for efficient data storage and retrieval
- Implement caching mechanisms for frequently accessed data
- Use multi-threading or asynchronous programming for performance improvements where applicable
- Ensure proper testing and validation of all features before release
- Follow semantic versioning for releases and updates
- Provide a roadmap for future development and feature additions
- Maintain a changelog for tracking changes and updates
- Document code with comments and use Rustdoc for generating documentation
- Use benchmarking tools to measure performance improvements
- Use secure coding practices to prevent vulnerabilities
- Provide support for different CSV formats and delimiters
- Implement data validation and sanitization for input files
- Use consistent naming conventions and coding styles throughout the codebase
- Leverage Rust's ownership and borrowing system for memory safety
- Use pattern matching for cleaner and more readable code
- Implement custom error types for better error handling
- Use Rust's macro system for code generation and reducing boilerplate
- Ensure compatibility with different Rust versions and toolchains
- Use Rust's testing framework for unit and integration tests
- Implement continuous monitoring and logging for production use
- Use Rust's async/await syntax for asynchronous programming
- Provide a comprehensive README file with installation and usage instructions
- Use Rust's ecosystem of crates for additional functionality and libraries
- Implement GitHub Actions for continuous integration and deployment
- Use Rust's traits and generics for code reuse and flexibility
- Follow the principles of clean code for maintainability and readability
- Use Rust's error handling mechanisms, such as Result and Option types
- Implement a modular architecture with separate modules for different functionalities
- Use Rust's standard library for common tasks and utilities
- Provide support for different character encodings in CSV files
- Use Rust's iterator traits for efficient data processing
- Implements quote safety in CSV parsing and writing
- Assumes comma separator by default for input files with file extension .csv
- Assumes tab separator by default for input files with file extension .tsv
