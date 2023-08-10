# debctl

debctl is a CLI tool for managing apt repositories. It's intended as a
replacement for
[add-apt-repository](https://manpages.debian.org/buster/software-properties-common/add-apt-repository.1.en.html)
and [apt-key](https://manpages.debian.org/testing/apt/apt-key.8.en.html) that
supports the newer
[deb822](https://manpages.debian.org/stretch/apt/sources.list.5.en.html#DEB822-STYLE_FORMAT)
format and implements modern best practices for managing signing keys.

*What's wrong with the existing tools?*

- `add-apt-repository` adds repository entries to `/etc/apt/sources.list`, which
  is being phased out by the newer deb822 format.
- `apt-key` is deprecated because it trusts signing keys for *all* apt
  repositories instead of just the ones they're meant to be signing.

This tool tries to encourage best practices while providing escape hatches for
doing weird stuff.

## Installation

Install [Rust](https://www.rust-lang.org/tools/install) and run:

```shell
cargo install debctl
```

This tool shells out to GnuPG for working with PGP keys, so you must have `gpg`
installed and available on your `PATH`.

## Examples

Let's add the [Docker apt
repository](https://docs.docker.com/engine/install/ubuntu/) to your system:

```shell
debctl new \
    --name docker \
    --uri https://download.docker.com/linux/ubuntu \
    --key https://download.docker.com/linux/ubuntu/gpg \
    --component stable
```

This downloads the signing key for the repository and installs it under
`/etc/apt/keyrings/`. It can fetch the signing key from a URL, a local file
path, or a keyserver and install it to a keyring or inline it into the
`.sources` file.

This command creates the repository entry at
`/etc/apt/sources.list.d/docker.sources`. Here's what that file looks like:

```
Enabled: yes
Types: deb
URIs: https://download.docker.com/linux/ubuntu
Suites: jammy
Components: stable
Signed-By: /etc/apt/keyrings/docker-archive-keyring.gpg
```

Most documentation for third-party apt repositories directs users to use
`add-apt-repository`. This tool accepts the old-style syntax used by
`add-apt-repository` and converts it to deb822 syntax:

```shell
debctl add \
    --name docker \
    --key https://download.docker.com/linux/ubuntu/gpg \
    'deb [arch=amd64 lang=en,de] https://download.docker.com/linux/ubuntu jammy stable'
```

Here's the file that command generates:

```
Enabled: yes
Types: deb
URIs: https://download.docker.com/linux/ubuntu
Suites: jammy
Components: stable
Signed-By: /etc/apt/keyrings/docker-archive-keyring.gpg
Architectures: amd64
Languages: en de
```

You can also convert existing old-style `.list` files to deb822-style `.sources`
files:

```shell
debctl convert --name docker
```

This replaces `/etc/apt/sources.list.d/docker.list` with
`/etc/apt/sources.list.d/docker.sources`.

Entries that are commented out in the `.list` file are included in the
`.sources` file, but with the `Enabled: no` option set. Regular comments in the
`.list` file are preserved and included in the `.sources` file.
