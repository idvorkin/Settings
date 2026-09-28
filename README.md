# Settings

A place to store my settings/dotFiles/etc, my oldest repository, and wow have I had fun here.

## Y lives in Alfred

The macOS Y implementation and tests now live in
[Igor Tools](https://github.com/idvorkin/alfred). Pull that repository into
`~/gits/alfred` before updating settings. Install uv with `brew install uv`.
The installed `y` command and `py/y.py` forward arguments to the bundled script.
For another checkout or imported workflow, set `IGOR_Y_SCRIPT` to its `y/y.py`.

## Mac apps

What a new Mac gets, each cloned into `~/gits/<repo>`:

| App                                                                   | What it is                                       | Install                                                                                 |
| --------------------------------------------------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------- |
| [Igor Tools](https://github.com/idvorkin/alfred)                      | Alfred workflows, including `y`                  | see above                                                                               |
| [yabai](https://github.com/idvorkin/yabai) (fork)                     | tiling window manager, with macOS 27 fixes       | [build from the fork](https://github.com/idvorkin/alfred#yabai-space-focus-on-macos-27) |
| [Window Sweaters](https://github.com/idvorkin/window-sweaters) (fork) | knitted window borders, plus Focused Window Only | `./scripts/build-app.sh && python3 scripts/install-local.py`                            |
| [LCD Timer](https://github.com/idvorkin/lcd-timer)                    | countdown that looks like a gym's LED clock      | `just install`                                                                          |
| [Magic Monitor](https://github.com/idvorkin/magic-monitor-native)     | camera practice mirror with instant replay       | `just install`                                                                          |
| [omnifocus_cli](https://github.com/idvorkin/omnifocus_cli)            | OmniFocus from the command line                  | `just global-install`                                                                   |

`just install` copies the app into `/Applications`; `just live-install` links it to the checkout's build instead,
so each rebuild is what opens.

## Y window numbers

Show numbered badges, then target a window by its displayed number:

```sh
y number
y 3
y 3 focus
y 3 close
y 3 zoom
y 3 half
y 3 third
y 3 sixty
y 3 move next
y 3 screenshot
```

`half`, `third`, and `sixty` set one-half, one-third, and two-thirds of a tiled
split. `move` accepts `next`, `prev`, or `recent`; `screenshot` copies the window.

`y 3` defaults to focus. `focus` activates that window; `close` closes that window directly. Alfred also
offers these actions after a number. The mapping lasts for the badge duration
plus five seconds; use `y number --seconds 20` for more time. If the mapping has
expired, run `y number` again. Numbers keep their original window targets even
after another window closes. `y 3 --help` lists the available actions.

## Normal linux

Mostly done via script, contained here:

```bash
cd ~
git clone https://github.com/idvorkin/settings
```

## Alpine Linux (using iSH)

I use ish as my ssh client, with some minor tweaks:

    cd ~
    apk add git vim openssh-client tig ranger zsh
    git clone https://github.com/idvorkin/settings
    ln -s ~/settings/shared/ssh_config ~/.ssh/config

## Windows

**Use WSL Instead**

1. Install chocolatey (new admin window)

   @powershell -NoProfile -ExecutionPolicy Bypass -Command "iex ((new-object net.webclient).DownloadString('https://chocolatey.org/install.ps1'))" && SET PATH=%PATH%;%ALLUSERSPROFILE%\chocolatey\bin

2. Install git (new admin window)

   cinst git

3. Clone settings (new admin window)

   cd \
   git clone https://github.com/idvorkin/settings

Touching Ignore
