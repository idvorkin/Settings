# Justfile for running tests

# Recipe to run the test for YouTube template
# nvim --headless -u nvim/tests/test_init.lua "PlenaryBustedDirectory nvim/tests/"
# Install locally and globally
install:
    @cd py && uv venv
    @cd py && . .venv/bin/activate
    @cd py && uv pip install --upgrade --editable .

global-install: install
    @cd py && uv tool install --force  --editable . --python $(which python3.12)

# Justfile

# Run all tests in the tests/ directory using minimal_init
test:
    @nvim --headless --clean -u nvim/min_test_init.lua -c "PlenaryBustedDirectory ./nvim/tests/ { minimal_init = 'nvim/min_test_init.lua' }" -c "qa!"


# Run a specific test file, use with just test_file FILE=your_test_file.lua
test_file FILE:
    @nvim --headless --clean -u nvim/tests/min_test_init.lua -c "PlenaryBustedFile {{FILE}}" -c "qa"



# Recipe to run the test for YouTube template with verbose output
test-verbose:

# Install the Python package globally using pipx
global_install:
    @cd py && pipxu install -e -f .
