# Justfile for running tests

# Recipe to run the test for YouTube template
# nvim --headless -u nvim/tests/test_init.lua "PlenaryBustedDirectory nvim/tests/"
test:
   nvim --headless -u nvim/tests/test_init.lua -c "PlenaryBustedDirectory nvim/tests/" -c "qa"




# Recipe to run the test for YouTube template with verbose output
test-verbose:
