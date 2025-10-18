# Instructions

## Overview

The objective is to use the Rust programming language to create a super fast and efficient command-line tool to manage CSV files of all sizes.  
The primary user personas for this tool are data engineers, machine learning (ML) engineers, and data scientists.
Use cases:
- Preparing data for use in training machine learning models.
- Preparing data for use in data science experiments
- Wrangling data for a wide array of situations in data engineering

Features of this tool include:
1. The ability to create a B-Tree index of one or more columns in a CSV file and store it in a new index file of extension .idx
2. The ability to select an index file for further processing of the CSV file.
3. The ability to sort the contents of the CSV file according to specified columns, including columns in an index file.
4. The ability to specify the direction of sort for both ascending and descending across multiple columns.
5. The ability to specify the column names and data types of a CSV file and store in a separate metadata file with the file extension .schema
6. The ability to probe a CSV file and propose the data types for each column and store them in a .meta file.
7. The ability to sort by those different data types, such as string, date, time, date-time, integer, float, decimal, GUID, and boolean.
8. The ability to output a sorted file to a new file using the -o argument for file output.
9. The ability to add row numbers to the start of each line of an output file.
10. The ability to select a subset of columns from the original file or from a new, sorted file.
11. The ability to derive or add new columns to the output of a file.
12. The ability to output a subset of rows based on search criteria by column and be able to process searches using data type specific criteria.
13. The ability to transform boolean values from various representations to standard true/false values and 1/0 values.
14. The ability to append data from one or more CSV files into a single output file
15. The ability to verify the integrity of a CSV file against its schema file.
16. The ability to verify the schema of multiple CSV file against a single schema file.
17. The ability to verify the schema of multiple CSV files before appending them into a single output file.
18. The ability to stream the processing of CSV files to minimize memory usage and maximize speed.
19. The ability to stdin and stdout for input and output of CSV data so that it can be chained with other command line utilities and itself.
20. The ability to project new columns, such as boolean columns, based on the evaluation of expressions against existing columns.
21. The ability to join two CSV files based on common columns, including support for inner joins, left joins, right joins, and full outer joins.
22. The ability to output to the command window the results of a filter operation without the need to create an output file in table format using elastic tabular formatting.
23. The ability to output a limited number of rows from the start of a CSV file for previewing purposes.
24. The ability to produce summary statistics for numeric columns in a CSV file, including count, mean, median, min, max, and standard deviation.
25. The ability to produce frequency counts for categorical columns in a CSV file.
26. The ability to specify which columns from the original input to NOT output to the output file.
27. The ability to install a released version of the executable as a binary for easy access from the command line from the cargo package store.
28. The ability to index a file on multiple combinations of columns and store multiple indexes for the same file and mixed sort directions (ascending/descending) per column.
29. The ability to list column names and data types as a list to the console output.
30. The ability to specify in the .schema file a mapping of existing column names to new column names to be used in all outputs from the file.
31. The ability to point the app at all files of the same file extension in a directory and verify each file against a .schema file schema definition including data type verification.
32. The ability to output the schema definition for a CSV file in a human-readable list format to the console output.
33. The ability to index all of the files in a directory matching a single schema file.
34. The ability to perform a union of multiple files that is able to deduplicate rows across multiple files and output to a single file.
35. The ability to union all of the files in a directory in a sorted order and split into multiple files based on either row count per file or file size.
36. The ability to consume a batch processing definition file in which all possible command-line arguments can be defined; file should be in JSON format.




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
