// Deliberately scoped dispatch probe, not a replacement for Y.
package main

import (
	"os"
	"os/exec"
)

func main() {
	args := os.Args[1:]
	if len(args) == 1 && args[0] == "noop" {
		return
	}
	if len(args) != 2 || args[0] != "focus" {
		os.Exit(2)
	}
	directions := map[string]string{"left": "west", "right": "east", "up": "north", "down": "south"}
	direction, ok := directions[args[1]]
	if !ok {
		os.Exit(2)
	}
	_, err := exec.Command(os.Getenv("Y_BENCH_YABAI"), "-m", "window", "--focus", direction).Output()
	if err != nil {
		if exit, ok := err.(*exec.ExitError); ok {
			os.Exit(exit.ExitCode())
		}
		os.Exit(1)
	}
}
