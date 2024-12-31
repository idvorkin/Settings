# Project Management

Add pyproject.toml, add missing dependancies to it, and new utiliies to it.

# Coding conventions used in this project


For CLIs, use a Typer app.
Use `ic` for logging.
Use Rich for pretty printing.
Use Loguru for logging.
Use Typer for CLI apps.
Use Pydantic for data validation.
Use types; when using types, prefer using built-ins like `foo | None` vs `foo: Optional[str]`.
When using Typer, use the latest syntax for arguments and options.

```python
    name: Annotated[Optional[str], typer.Argument()] = None
    def main(name: Annotated[str, typer.Argument()] = "Wade Wilson"):
    lastname: Annotated[str, typer.Option(help="Last name of person to greet.")] = "",
    formal: Annotated[bool, typer.Option(help="Say hi formally.")] = False,
```

### When creating a new utility

Have the header to be runnable be #!python3
Include the new utility in pyproject
Ask user to /run chmod on it

### Code Style

Prefer returning from a function vs nesting ifs.
Prefer descriptive variable names over comments.
Avoid nesting ifs, return from functions as soon as you can

### Types

Use types whenever possible.
Use the latest syntax for types (3.12)
Don't use tuples, define pydantic types for return values. Call Them FunctionReturn for the function name
<examples>
For a Single Item Return
Function: get_user_profile()
Return Type: UserProfileResponse
For Multiple Items
Function: list_users()
Return Type: UserListResponse or PaginatedUsersResponse
For Aggregated Data
Function: get_sales_summary()
Return Type: SalesSummaryResult
For Nested or Composite Data
Function: get_order_details()
Return Type: OrderDetailsResponse (which may include nested models like ProductInfo or ShippingDetails).
</examples>

### Testing

When possible update the tests to reflect the new changes.
Tests are in the test directory.

#### Test Organization and Structure
- Use pytest as the testing framework
- Organize tests into three categories:
  - `tests/unit/`: For testing individual components in isolation
  - `tests/integration/`: For testing component interactions
  - `tests/e2e/`: For end-to-end testing of complete features

#### Test Configuration
- All test dependencies should be listed in pyproject.toml
- Required test packages include:
  - pytest
  - pytest-xdist (for parallel execution)
  - pytest-cov (for coverage reporting)
  - pytest-asyncio (if testing async code)

#### Test Execution
- Tests should be designed to run in parallel using pytest-xdist
- Run tests with: `pytest -n auto` for automatic parallel execution
- Use markers to categorize tests:
  ```python
  @pytest.mark.unit
  @pytest.mark.integration
  @pytest.mark.e2e
  ```

#### Test Best Practices
- Use fixtures for test setup and teardown
- Keep tests independent and isolated
- Follow AAA pattern (Arrange, Act, Assert)
- Use meaningful test names that describe the scenario being tested
- Avoid test interdependencies
- Use parametrize for testing multiple scenarios

### Adding Libraries

When adding a new library, ensure it's in pyproject.toml

### Adding New scripts

When adding new scripts, be sure to add them to the scripts section in pyproject.toml

### When running python,

