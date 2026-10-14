# Ralph-loop Feature Manual

This guide describes how to use the Ralph-loop feature, an autonomous task processing capability integrated directly into KubiRAG. This feature allows the system to work through a list of tasks iteratively, generating clean context between each iteration and maintaining progress through persistent state.

## Table of Contents

- [Overview](#overview)
- [Getting Started](#getting-started)
- [Commands](#commands)
  - [`/ralph-loop` - Start Ralph-loop Mode](#ralph-loop---start-ralph-loop-mode)
  - [`/ralph-loop-config` - Configure Ralph-loop](#ralph-loop-config---configure-ralph-loop)
- [Usage Examples](#usage-examples)
- [Task Categories](#task-categories)
- [Quality Gates](#quality-gates)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)

---

## Overview

The Ralph-loop feature is an autonomous task processing system that:

- **Iterates through tasks**: Processes a list of tasks one by one
- **Maintains clean context**: Creates fresh context between iterations to avoid contamination
- **Tracks progress**: Persists state and progress through `progress.md`
- **Validates quality**: Uses configurable quality gates between iterations
- **Integrates with RAG**: Leverages existing RAG components for task execution

### Key Benefits

- **Autonomous Development**: Let the system work through your backlog while you focus on other tasks
- **Clean Context**: Avoid context window degradation by starting fresh each iteration
- **Quality Assurance**: Built-in validation and quality checks
- **Progress Tracking**: Detailed progress logging and state persistence
- **Integration**: Deep integration with existing KubiRAG components

---

## Getting Started

### Prerequisites

1. **Java 21**: Required runtime environment
2. **Ollama**: Local LLM server for task execution
3. **Maven**: For building and dependency management
4. **Project Structure**: Ensure your project follows standard Maven structure

### Quick Start

```bash
# Build the project
mvn clean package

# Start Ralph-loop mode
./qbrag ralph-loop --tasks "test-coverage,documentation,performance"
```

---

## Commands

### `/ralph-loop` - Start Ralph-loop Mode

Starts the Ralph-loop autonomous task processing mode.

#### Syntax

```
/ralph-loop [options]
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--tasks` | Comma-separated list of tasks to process | "all" |
| `--iterations` | Maximum number of iterations | 100 |
| `--auto` | Automatic mode (no user intervention) | false |
| `--debug` | Enable debug logging | false |
| `--resume` | Resume from last saved state | false |
| `--quality` | Quality threshold (0.0-1.0) | 0.8 |
| `--help` | Show help message | |

#### Examples

```bash
# Start with specific tasks
/ralph-loop --tasks "test-coverage,documentation,performance"

# Start with maximum iterations
/ralph-loop --iterations 50

# Start in automatic mode
/ralph-loop --auto

# Start with debug logging
/ralph-loop --debug

# Resume from previous state
/ralph-loop --resume

# Start with custom quality threshold
/ralph-loop --quality 0.9
```

### `/ralph-loop-config` - Configure Ralph-loop

Configures Ralph-loop settings and parameters.

#### Syntax

```
/ralph-loop-config [options]
```

#### Options

| Option | Description |
|--------|-------------|
| `--show` | Show current configuration |
| `--list-tasks` | List available task types |
| `--help` | Show help message |

#### Examples

```bash
# Show current configuration
/ralph-loop-config --show

# List available task types
/ralph-loop-config --list-tasks
```

---

## Usage Examples

### Example 1: Basic Ralph-loop Execution

```bash
# Start Ralph-loop with default settings
./qbrag ralph-loop

# Output:
Starting Ralph-loop mode...
Loading tasks from progress.md...
Found 5 incomplete tasks

## Iteration 1: Add unit tests for DocumentChunkingService
Creating clean context for iteration...
Executing task: Add unit tests for DocumentChunkingService
Generating test cases...
Running tests...
Quality check passed (95%)
Saving progress...

## Iteration 2: Update API documentation for RAGSystem
Creating clean context for iteration...
Executing task: Update API documentation for RAGSystem
Generating documentation...
Validating documentation...
Quality check passed (92%)
Saving progress...

## Iteration 3: Optimize vector store performance
Creating clean context for iteration...
Executing task: Optimize vector store performance
Analyzing performance bottlenecks...
Implementing optimizations...
Benchmarking improvements...
Quality check passed (98%)
Saving progress...

All tasks completed successfully!
Total iterations: 3
Average quality score: 95%
```

### Example 2: Targeted Task Processing

```bash
# Start Ralph-loop with specific tasks
./qbrag ralph-loop --tasks "test-coverage,documentation"

# Output:
Starting Ralph-loop mode with tasks: test-coverage,documentation

## Iteration 1: Add unit tests for DocumentChunkingService
Task completed successfully

## Iteration 2: Update API documentation for RAGSystem
Task completed successfully

All specified tasks completed!
Total iterations: 2
Average quality score: 94%
```

### Example 3: Automatic Mode

```bash
# Start Ralph-loop in automatic mode
./qbrag ralph-loop --auto

# Output:
Starting Ralph-loop mode in automatic mode...
Processing all available tasks...

## Iteration 1: Add unit tests for DocumentChunkingService
Task completed successfully

## Iteration 2: Update API documentation for RAGSystem
Task completed successfully

## Iteration 3: Optimize vector store performance
Task completed successfully

## Iteration 4: Add error handling to RAGSystem
Task completed successfully

## Iteration 5: Create performance benchmarks
Task completed successfully

All tasks completed successfully!
Total iterations: 5
Average quality score: 96%
```

### Example 4: Resume from Previous State

```bash
# Resume from previous state
./qbrag ralph-loop --resume

# Output:
Resuming Ralph-loop from previous state...
Found 3 incomplete tasks from previous session

## Iteration 4: Add error handling to RAGSystem
Resuming task...
Task completed successfully

## Iteration 5: Create performance benchmarks
Task completed successfully

All tasks completed successfully!
Total iterations: 2 (resumed)
Average quality score: 95%
```

---

## Task Categories

### Development Tasks

#### Code Generation
- **Implement new features**: Create new functionality based on requirements
- **Refactor existing code**: Improve code structure and maintainability
- **Bug fixes**: Identify and fix issues in the codebase

#### Testing
- **Add unit tests**: Create comprehensive unit test coverage
- **Integration tests**: Test component interactions
- **Performance tests**: Benchmark and optimize performance

#### Quality Assurance
- **Code quality**: Improve code structure and best practices
- **Error handling**: Enhance error detection and recovery
- **Security**: Add security measures and validation

### Documentation Tasks

#### API Documentation
- **Generate API docs**: Create comprehensive API documentation
- **Update README**: Keep project documentation current
- **Create guides**: Write user guides and tutorials

#### Code Documentation
- **Add comments**: Improve code documentation and clarity
- **Update examples**: Create and maintain code examples
- **Document architecture**: Explain system architecture and design

### Performance Tasks

#### Optimization
- **Algorithm optimization**: Improve algorithm efficiency
- **Resource usage**: Optimize memory and CPU usage
- **Database optimization**: Improve database query performance

#### Benchmarking
- **Performance benchmarks**: Create comprehensive benchmarks
- **Load testing**: Test system performance under load
- **Monitoring**: Add performance monitoring and metrics

### Quality Tasks

#### Code Quality
- **Static analysis**: Run code quality analysis tools
- **Code review**: Implement automated code review
- **Best practices**: Ensure adherence to coding standards

#### Error Handling
- **Exception handling**: Improve exception handling and recovery
- **Validation**: Add input validation and sanitization
- **Logging**: Enhance logging and debugging capabilities

---

## Quality Gates

### Quality Metrics

The Ralph-loop system uses multiple quality metrics to validate task completion:

#### Completeness Metrics
- **Task completion percentage**: How much of the task was completed
- **Feature implementation coverage**: Percentage of required features implemented
- **Test coverage**: Percentage of code covered by tests

#### Accuracy Metrics
- **Code correctness**: Validation of code functionality
- **Documentation accuracy**: Accuracy of generated documentation
- **Performance benchmarks**: Achievement of performance targets

#### Relevance Metrics
- **Task relevance**: How relevant the task is to project goals
- **Feature usefulness**: Assessment of feature usefulness
- **Documentation relevance**: Relevance of documentation to users

#### Consistency Metrics
- **Code style consistency**: Adherence to coding standards
- **Documentation consistency**: Consistency across documentation
- **API consistency**: Consistency of API design and implementation

### Quality Thresholds

#### Default Thresholds
- **Completeness**: 80% minimum
- **Accuracy**: 85% minimum
- **Relevance**: 70% minimum
- **Consistency**: 75% minimum

#### Custom Thresholds

```bash
# Start with custom quality threshold
./qbrag ralph-loop --quality 0.9
```

### Quality Validation

#### Validation Process
1. **Task completion check**: Verify all task requirements are met
2. **Code validation**: Run static analysis and tests
3. **Documentation validation**: Check documentation quality and accuracy
4. **Performance validation**: Verify performance benchmarks
5. **Quality scoring**: Calculate overall quality score

#### Quality Results

```
## Quality Check Results

Task: Add unit tests for DocumentChunkingService
- Completeness: 95% (All test cases implemented)
- Accuracy: 98% (All tests pass)
- Relevance: 90% (Highly relevant to project)
- Consistency: 92% (Follows project standards)
- **Overall Quality Score: 94%**
```

---

## Configuration

### Configuration File

The Ralph-loop system uses a configuration file to manage settings:

```json
{
  "ralphLoop": {
    "maxIterations": 100,
    "qualityThreshold": 0.8,
    "autoMode": false,
    "debugMode": false,
    "resumeEnabled": true,
    "taskCategories": ["development", "documentation", "performance", "quality"],
    "qualityMetrics": {
      "completenessWeight": 0.3,
      "accuracyWeight": 0.3,
      "relevanceWeight": 0.2,
      "consistencyWeight": 0.2
    },
    "contextSettings": {
      "cleanContext": true,
      "contextSize": 4000,
      "contextIsolation": true
    }
  }
}
```

### Configuration Options

#### General Settings
- **maxIterations**: Maximum number of iterations to run
- **qualityThreshold**: Minimum quality score to continue
- **autoMode**: Enable automatic mode (no user intervention)
- **debugMode**: Enable debug logging and verbose output
- **resumeEnabled**: Allow resuming from previous state

#### Task Settings
- **taskCategories**: Categories of tasks to process
- **qualityMetrics**: Weights for different quality metrics

#### Context Settings
- **cleanContext**: Create fresh context for each iteration
- **contextSize**: Maximum context size for iterations
- **contextIsolation**: Isolate context between iterations

### Environment Variables

```bash
# Set Ralph-loop configuration via environment variables
export RALPH_LOOP_MAX_ITERATIONS=50
export RALPH_LOOP_QUALITY_THRESHOLD=0.9
export RALPH_LOOP_AUTO_MODE=true
```

---

## Troubleshooting

### Common Issues

#### Issue 1: "No tasks found"

**Problem**: Ralph-loop cannot find any tasks to process

**Solutions**:
1. **Check progress.md**: Verify the progress file exists and contains tasks
2. **Create tasks**: Add tasks to the progress file manually
3. **Use default tasks**: Start with `--tasks "all"` to use default task list

#### Issue 2: "Quality check failed"

**Problem**: Task quality score is below threshold

**Solutions**:
1. **Review quality criteria**: Check the specific quality metrics that failed
2. **Adjust threshold**: Lower the quality threshold if needed
3. **Improve task**: Modify the task to meet quality requirements
4. **Skip quality check**: Use `--quality 0.0` to skip quality validation

#### Issue 3: "Context contamination"

**Problem**: Previous iteration context is affecting current iteration

**Solutions**:
1. **Enable clean context**: Ensure `cleanContext` is set to true
2. **Increase context isolation**: Set `contextIsolation` to true
3. **Check context size**: Verify context size is appropriate
4. **Review context management**: Check context management implementation

#### Issue 4: "State persistence failed"

**Problem**: Unable to save or restore state

**Solutions**:
1. **Check file permissions**: Ensure write permissions for progress.md
2. **Verify file location**: Check the file path and location
3. **Check disk space**: Ensure sufficient disk space
4. **Review state management**: Check state persistence implementation

#### Issue 5: "Task execution timeout"

**Problem**: Task execution takes too long

**Solutions**:
1. **Increase timeout**: Adjust task execution timeout settings
2. **Optimize task**: Improve task efficiency and performance
3. **Break down task**: Split large tasks into smaller ones
4. **Check resources**: Verify system resources and availability

### Debug Mode

Enable debug mode to get detailed information about Ralph-loop execution:

```bash
# Start with debug mode
./qbrag ralph-loop --debug

# Output includes:
- Detailed task execution logs
- Context management information
- Quality gate validation details
- State persistence information
- Performance metrics
```

### Log Files

Ralph-loop generates detailed log files for troubleshooting:

- **ralph-loop.log**: Main Ralph-loop execution logs
- **quality-gates.log**: Quality gate validation logs
- **context-management.log**: Context management logs
- **state-persistence.log**: State persistence logs

### Performance Monitoring

Monitor Ralph-loop performance using built-in metrics:

```bash
# Show performance metrics
./qbrag ralph-loop --show-metrics

# Metrics include:
- Task execution time
- Quality score trends
- Context management performance
- State persistence efficiency
- Resource usage
```

---

## Best Practices

### Task Management

1. **Start small**: Begin with simple tasks and gradually increase complexity
2. **Clear requirements**: Ensure tasks have clear, well-defined requirements
3. **Prioritize tasks**: Order tasks by importance and dependencies
4. **Monitor progress**: Regularly check progress and quality metrics

### Quality Assurance

1. **Set appropriate thresholds**: Configure quality thresholds that balance quality and progress
2. **Regular validation**: Validate task completion and quality regularly
3. **Continuous improvement**: Use quality feedback to improve task definitions
4. **Document issues**: Keep track of quality issues and resolutions

### Context Management

1. **Clean context**: Always use clean context between iterations
2. **Context size**: Optimize context size for performance and quality
3. **Context isolation**: Ensure proper context isolation between tasks
4. **Context transfer**: Use context transfer only when beneficial

### Performance Optimization

1. **Resource monitoring**: Monitor system resources during execution
2. **Task optimization**: Optimize task execution for better performance
3. **Batch processing**: Process similar tasks together when possible
4. **Parallel execution**: Use parallel execution for independent tasks

---

## Advanced Features

### Custom Task Types

Create custom task types by implementing the `RalphTask` interface:

```kotlin
class CustomTask : RalphTask {
    override fun execute(context: TaskContext): TaskResult {
        // Custom task implementation
    }
    
    override fun validate(result: TaskResult): QualityScore {
        // Custom validation logic
    }
}
```

### Task Dependencies

Define task dependencies to ensure proper execution order:

```kotlin
val taskDependencies = mapOf(
    "test-coverage" to listOf("code-quality"),
    "documentation" to listOf("api-design"),
    "performance" to listOf("test-coverage", "documentation")
)
```

### Parallel Execution

Enable parallel execution for independent tasks:

```bash
# Start with parallel execution
./qbrag ralph-loop --parallel --max-threads 4
```

### Custom Quality Gates

Implement custom quality gates for specific task types:

```kotlin
class CustomQualityGate : QualityGate {
    override fun validate(task: Task, result: TaskResult): QualityScore {
        // Custom validation logic
    }
}
```

---

## Integration with Existing Features

### Integration with RAG System

The Ralph-loop feature integrates seamlessly with the existing RAG system:

- **Document processing**: Use existing document processing capabilities
- **Vector store**: Leverage vector store for semantic search
- **LLM integration**: Use existing LLM integration for task execution
- **Search capabilities**: Utilize existing search functionality

### Integration with Voice System

Ralph-loop can work with the voice system for hands-free operation:

```bash
# Start Ralph-loop with voice control
./qbrag ralph-loop --voice
```

### Integration with API

Control Ralph-loop programmatically via the API:

```kotlin
val ralphLoop = RalphLoopClient()
ralphLoop.start(tasks = listOf("test-coverage", "documentation"))
ralphLoop.getStatus()
ralphLoop.stop()
```

---

## Configuration Management (`/ralph-config`)

The `/ralph-config` command allows you to view and edit Ralph-loop configuration without manually editing JSON files.

### Subcommands

| Subcommand | Description |
|---|---|
| `show` | Display current configuration |
| `set <key> <value>` | Update a configuration value |
| `reset` | Reset to default configuration |
| `validate` | Validate current configuration |
| `help` | Show command help |

### Examples

```
# Show current config
/ralph-config show

# Increase iteration limit
/ralph-config set maxIterations 50

# Raise quality threshold
/ralph-config set qualityThreshold 0.9

# Enable debug output
/ralph-config set debugMode true

# Reset to defaults
/ralph-config reset
```

### Configurable Keys

| Key | Type | Default | Description |
|---|---|---|---|
| `maxIterations` | int | 100 | Maximum number of loop iterations |
| `autoMode` | bool | false | Skip user confirmation prompts |
| `debugMode` | bool | false | Enable verbose debug output |
| `resumeEnabled` | bool | true | Allow `--resume` flag |
| `qualityThreshold` | 0.0–1.0 | 0.8 | Minimum overall quality score |

Config is persisted in `ralph-loop-config.json` in the working directory.

---

## Context Templates

Context templates provide consistent, task-type-specific context structures that improve execution quality.

### Built-in Templates

| Template | Task Category | Key content |
|---|---|---|
| `development` | DEVELOPMENT | Kotlin conventions, JUnit 4, coverage requirements |
| `documentation` | DOCUMENTATION | Structure requirements, audience targeting |
| `performance` | PERFORMANCE | Profiling-first guidelines, benchmark requirements |
| `quality` | QUALITY | Code smell detection, acceptance criteria |

### Using Templates Programmatically

```kotlin
val contextManager = ContextManager()
contextManager.registerTemplate("development", DevelopmentContextTemplate)

val context = contextManager.applyTemplate("development", mapOf(
    "task" to "Add tests for DocumentChunkingService",
    "target" to "DocumentChunkingService.kt",
    "coverage" to "90%"
))
```

### Custom Templates

```kotlin
val myTemplate = CustomContextTemplate(
    name = "my-template",
    description = "Custom context for domain-specific tasks",
    templateText = "### Task: {{task}}\nDomain: {{domain}}\nConstraints: {{constraints}}"
)
contextManager.registerTemplate("my-template", myTemplate)
```

---

## Context Analytics

Monitor context usage to optimise performance:

```
# In code:
val analytics = ContextAnalytics(contextManager)
println(analytics.generateReport())
analytics.getOptimisationSuggestions().forEach { println("- $it") }
```

The report covers:
- Number of contexts created and archived
- Cache hit rate
- Average context size vs configured limit
- Archive disk usage
- Optimisation recommendations

---

## Conclusion

The Ralph-loop feature provides a powerful autonomous task processing capability that integrates seamlessly with the KubiRAG system. By leveraging clean context management, quality gates, and persistent state, it enables efficient and reliable task execution while maintaining high quality standards.

Whether you're working on code generation, testing, documentation, or performance optimization, Ralph-loop can help you systematically work through your task list while ensuring quality and consistency throughout the development process.

For more information or support, please refer to the [Ralph-loop GitHub repository](https://github.com/your-repo/kubirag) or open an issue in the [project tracker](https://github.com/your-repo/kubirag/issues).