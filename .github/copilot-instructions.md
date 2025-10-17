# Instructions

## Overview

The objective is to use the Rust programming language to create a super fast and efficient command-line tool to manage CSV files of all sizes.  
Features of this tool include:
1. The ability to create a B-Tree index of one or more columns in a CSV file and store it in a new index file of extension .idx
2. The ability to select an index file for further processing of the CSV file.
3. The ability to sort the contents of the CSV file according to specified columns, including columns in an index file.
4. The ability to specify the direction of sort for both ascending and descending across multiple columns.
5. The ability to specify the data types of a CSV file and store in a separate metadata file with the file extension .meta
6. The ability to probe a CSV file and propose the data types for each column and store them in a .meta file.
7. The ability to sort by those different data types, such as string, date, number.
8. The ability to output a sorted file to a new file using the -o argument for file output.
9. The ability to add row numbers to the start of each line of an output file.
10. The ability to select a subset of columns from the original file or from a new, sorted file.
11. The ability to derive or add new columns to the output of a file.
12. The ability to output a subset of rows based on search criteria by column and be able to process searches using data type specific criteria.

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
