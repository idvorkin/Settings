# Justfile for running tests

# Recipe to run the test for YouTube template
test:
    @busted nvim/test_*.lua

# Recipe to run the test for YouTube template with verbose output
test-verbose:
    @busted --verbose nvim/test_*.lua
