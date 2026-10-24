# The plugin store's admin page

A React app the store server embeds. `bun run build` writes it to `../assets/admin/`, and the
build is committed: building the server never needs bun, and CI checks the two agree.

```sh
bun install
bun run build      # after any change here, then rebuild the server
```

To work on it against a running store:

```sh
anistream store serve ./my-store --url http://127.0.0.1:7878
STORE=http://127.0.0.1:7878 bun run dev
anistream store admin ./my-store   # copy the #login=… part onto the dev server's address
```

The same build is the landing page at the store's root, which the server serves with a `<base>` at
`admin/` and `data-page="home"`. The dev server shows it at `/?home`.

The dev server forwards `/v1` to the store and presents the store's own origin, because the store
only accepts changes from the address it serves the page at.

To see the form that claims a store nobody owns yet, serve a fresh store with
`--setup-code <code>` and open the page before anyone joins it.
