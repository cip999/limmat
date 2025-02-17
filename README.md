# Limmat: Local Immediate Automated Testing

Limmat watches a Git branch for changes, and runs tests on every commit, in
parallel. It's a bit like having good CI, but it's all local so you don't need
to figure out infrastructure, and you get feedback faster.

It gives you a live web (and terminal) UI to show you which commits are passing
or failing each test:

![screenshot of UI](docs/assets/screenshot.png)

Clicking on the test results ([including in the
terminal](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda))
will take you to the logs.

## Installation

> [!NOTE]
> Limmat works on Linux on x86. It _probably_ works on other architectures too.
> It's also been reported to work on MacOS.

### From crates.io

[Install
Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) then:

```sh
cargo install limmat
```

### From GitHub Releases

There are pre-built Linux x86 binaries in the [GitHub
Releases](https://github.com/bjackman/limmat/releases/tag/v0.2.1).

If you prefer to have the tool tracked by your package manager, you can download
a `.deb` from there and install it with `dpkg -i $pkg.deb`. Or you can just
download the raw binary, it has no dependencies.

## Usage

Write a config file (details [below](#configuration)) in `limmat.toml` or `.limmat.toml`, and
if the branch you're working on is based on `origin/master`, run this from the root
of your repository:

```sh
limmat watch origin/master
```

Limmat will start testing every commit in the range `origin/master..HEAD`.
Meanwhile, it watches your repository for commits being added to or removed from
that range and spawns new tests or cancels them as needed to get you your
feedback as soon as possible.

By default tests are run in separate [Git worktrees](https://git-scm.com/docs/git-worktree).

If you don't want to store the config in the repo, put it elsewhere and point to
it with `--config`. Alternatively you can run Limmat from a different directory
and point to the repository with `--repo`.

## Configuration

Configuration is in [TOML](https://toml.io/en/). Let's start with an example,
here's how you might configure a Rust project (it's a reduced version of [this
repository's own config](limmat.toml)):

```toml
# Check that the formatting is correct
[[tests]]
name = "fmt"
command = "cargo fmt --check"

# Check that the tests pass
[[tests]]
name = "test"
command = "cargo test"
```

If the comand is a string, it's executed via the shell. If you want direct
control of the command then pass it as a list:

```toml
[[tests]]
name = "test"
command = ["cargo", "test"]
```

### Writing the test command

The test command's job is to produce a zero (success) or nonzero (failure) status
code. By default, it's run from the root directory of a copy of the repository,
with the commit to be tested already checked out.

If your test command is nontrivial, test it with `limmat test
$test_name`. This runs it immediately in the main worktree and print its output
directly to your terminal.

> [!WARNING]
> Limmat doesn't currently lock the result database. If you run `limmat test`
> while `limmat watch` is running, confusing things might happen. (This is a
> bug, it should be fixed in an upcoming version!).

> [!WARNING]
> Limmat doesn't clean the source tree for you, it just does `git checkout`. If
> your test command can't be trusted to work in a dirty worktree (for example,
> if you have janky Makefiles) you might want it to start with something like
> `git clean -fdx`. **But**, watch out, because when you run that via `limmat test`,
> it will wipe out any untracked files from your main worktree.

If your test command doesn't actually need to access the codebase, for example
if it only cares about the commit message, you can set `needs_worktree = false`.
In that case it will run in your main worktree, and the commit it needs to test
will be passed in the [environment](#job-environment) as `$LIMMAT_COMMIT`.

> [!NOTE]
> Tests configured with `command` are currently hard-coded to use Bash as the
> shell. There's no good reason for this it's just a silly limitation of the
> implementation.

When the test is no longer needed (usually because the commit is no longer in
the range being watched), the test comamnd's process group will receive
`SIGTERM`. It should try to shut down promptly so that the worktree can be
reused for another test. If it doesn't shut down after a timeout then it will
receive `SIGKILL` instead. You can configure the timeout by setting
`shutdown_grace_period_s` in seconds (default 60).

### Caching

Results are stored in a database, and by default Limmat won't run a test again
if there's a result in the database for that commit.

You can disable that behaviour for a test by setting `cache = "no_caching"`;
then when Limmat restarts it will re-run all instances of that test.

Alternatively, you can crank the caching _up_ by setting `cache = "by_tree"`.
That means Limmat won't re-run tests unless the actual repository contents
change - for example changes to the commit message won't invalidate cache
results.

If the test is terminated by a signal, it isn't considered to have produced a
result: instead of "success" or "failure" it's an "error". Errors aren't cached.

> [!TIP]
> You can use this as a hack to prevent environmental failures from
> being stored as test failures. For example, in my own scripts I use `kill
> -SIGUSR1 $$` if no devices are available in my company's test lab. In a later
> version I'd like to formalize this hack as a feature, using designated exit
> codes instead of signal-termination.

The configuration for each test and its dependencies are hashed, and if this
hash changes then the database entry is invalidated.

> [!WARNING]
> If your test script uses config files that aren't checked into your repository,
> Limmat doesn't know about that and can't hash those files. It's up to you
> to determine if your scripts are "hermetic" - if they aren't you probably just want 
> to set `cache = "no_caching"`.

### Resources

If you're still reading, you probably have a lot of tests to run, otherwise you
wouldn't find Limmat useful. So the system needs a way to throttle the
parallelism to avoid gobbling resources. The most obvious source of throttling is
the worktrees. If your tests need one - i.e. if you haven't set `needs_worktee =
false` - then those tests can only be parallelised up to the `num_worktrees`
value set in your config (default: 8). But there's also more flexible throttling
available.

To use this, define `resources` globally (separately from `tests`) in your
config file, for example:

```toml
[[resources]]
name = "pokemon"
tokens = ["moltres", "articuno", "zapdos"]
```

Now a can refer to this resource, and it won't be run until Limmat can
allocate a Pokemon for it:

```toml
[[tests]]
name = "test_with_pokemon"
resources = ["pokemon"]
command = "./test_with_pokemon.sh --pokemon=$LIMMAT_RESOURCE_pokemon"
```

As you can see, resource values are passed in the
[environment](#job-environment) to the test command.

Resources don't need to have values, they can also just be anonymous tokens for
limiting parallelism:

```toml
resources = [
    "singular_resource", # If you don't specify a count, it defaults to 1.
    { "name" = "threeple_resource", count = 3 },
]
```

### Test dependencies

Tests can depend on other tests, in which case Limmat won't run them until the
dependency tests have succeeded for the corresponding commit:

```toml
[[tests]]
name = "build-prod"
command = "make"

[[tests]]
name = "run-tests"
# No point in trying to run the tests if the prod binary doesn't compile
depends_on = ["build-prod"]
command = "run_tests.sh"
```

Tests aren't currently given a practical way to access the _output_ of their
dependency jobs, so this has limited use-cases right now, primarily:

1. You can prioritise the test jobs that give you faster feedback.
2. If you have some totally out-of-band way to pass output between test jobs, as
   is the case in the [advanced example](#advanced-example).

### Reference

#### Config file

The JSON Schema is [available in the
repo](https://github.com/bjackman/limmat/blob/master/limmat.schema.json). (The
configuration is is TOML, but TOML and JSON are equivalent for our purposes
here. Limmat might accept JSON directly in a later version, and maybe other
formats like YAML). There are online viewers for reading JSON Schemata more
easily, try [viewing it in Atlassian's tool
here](https://json-schema.app/view/%23?url=https%3A%2F%2Fraw.githubusercontent.com%2Fbjackman%2Flimmat%2Frefs%2Fheads%2Fmaster%2Flimmat.schema.json).
[Contributions are
welcome](https://github.com/bjackman/limmat/commit/3181929c0f9031dbe9b13ad07a52b66f2f3439a4)
for static HTML documentation.

#### Job environment

These environment variables are passed to your job.

| Name                                  | Value                                                                                     |
| ------------------------------------- | ----------------------------------------------------------------------------------------- |
| `LIMMAT_ORIGIN`                       | Path of the main repository worktree (i.e. `--repo`).                                     |
| `LIMMAT_COMMIT`                       | Hash of the commit to be tested.                                                          |
| `LIMMAT_RESOURCE_<resource_name>_<n>` | Values for [resources](#resources) used by the test.                                      |
| `LIMMAT_RESOURCE_<resource_name>`     | If the test only uses one of a resource, shortand for `LIMMAT_RESOURCE_<resource_name>_0` |

### Advanced example

Here's a fictionalised example showing all the features in use at once, based on
the configuration I use for my Linux kernel development at Google.

```toml
# Don't DoS the remote build service
[[resources]]
name = "build_service"
count = 8

# Physical hosts to run tests on, with two different CPU microarchtectures.
[[resources]]
name = "milan_host"
tokens = ["milan-a8", "milan-x3"]
[[resources]]
name = "skylake_host"
tokens = ["skylake-a8", "skylake-x3"]

# Build a kernel package using the remote build service.
[[tests]]
name = "build_remote"
resources = ["build_service"]
command = "build_remote_kernel.sh --commit=$LIMMAT_COMMIT"
# build_remote_kernel.sh doesn't actually need to access the code locally, it
# just pushes the commit to a Git remote.
requires_worktree = false

# Let me know when I break the build for Arm.
[[tests]]
name = "build_remote"
resources = ["build_service"]
command = "build_remote_kernel.sh --commit=$LIMMAT_COMMIT --arch=arm64"
requires_worktree = false


# Also check that the build works with the normal kernel build system.
[[tests]]
name = "kbuild"
command = """
set -e

# The kernel's Makefiles are normally pretty good, but just in case...
git clean -fdx

make -j defconfig
make -j16 vmlinux
"""

# Check the kernel boots on an AMD CPU.
[[tests]]
name = "boot_milan"
resources = ["milan_host"]
command = "boot_remote_kernel.sh --commit=$LIMMAT_COMMIT --host=$LIMMAT_RESOURCE_milan_host"
# boot_remote_kernel.sh will just use the kernel built be build_remote_kernel.sh
# so it doesn't need to access anything locally.
requires_worktree = false
# But we must only run this test once that remote build is finished.
depends_on = ["build_remote"]

# Same as above, but on an Intel CPU.  This simplified example isn't bad, but
# this can get pretty verbose in more complex cases.  Perhaps a later version
# will optionally support a more flexible config language like Cue, to allow
# DRY configuration.
[[tests]]
name = "boot_skylake"
resources = ["skylake_host"]
command = "boot_remote_kernel.sh --commit=$LIMMAT_COMMIT --host=$LIMMAT_RESOURCE_skylake_host"
requires_worktree = false
depends_on = ["build_remote"]

# Run KUnit tests
[[tests]]
name = "kunit"
command = "./tools/testing/kunit/kunit.py run --kunitconfig=lib/kunit/.kunitconfig"
```
