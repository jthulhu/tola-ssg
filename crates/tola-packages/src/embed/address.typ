// @tola/address:0.0.0 - slugs, routes, output paths, and browser URLs
//
// Choose a route for a document, turn it into an output for `#document`, then turn that output
// into a browser URL for links. These values describe different parts of publishing the same file:
//
// | Value | Meaning | Example |
// | --- | --- | --- |
// | route | A decoded logical address from the site root; starts with `/` | `/Hello World/` |
// | output | The file path written under `build.publish-dir`; no leading `/` or deployment prefix | `Hello World/index.html` |
// | browser URL | The address used in a link; percent-encoded, with the deployment prefix and optionally an origin | `/docs/Hello%20World/` |
//
// A route's leading `/` means the root of this site. Its trailing slash distinguishes a directory
// from a file: `/guide/` gives `guide/index.html`, while `/guide` gives the file `guide`.
// `output-of` makes this conversion; `route-of` returns the canonical route of an output, so
// `guide/index.html` becomes `/guide/`. An explicit route `/guide/index.html` names the same output
// as `/guide/` and canonicalizes to `/guide/` too.
//
// A source path names an input file, such as `content/hello.typ`; it is separate from all three
// addresses. The output passed to `#document` decides where that document is published. Use
// `join-route` to compose decoded route segments, and `slugify` first when those segments should
// be slugs (`slugify("Hello World")` gives `"hello-world"`).
//
// Here the route and output retain the space. Only the browser URL encodes it and adds `/docs/`:
//
// ```typst
// #import "@tola/address:0.0.0": output-of, route-of, output-url, route-url
// #let route = "/Hello World/"
// #let output = output-of(route)
// #let url = output-url(output, base-path: "/docs/")
// #assert.eq(output, "Hello World/index.html")
// #assert.eq(route-of(output), route)
// #assert.eq(url, "/docs/Hello%20World/")
// #assert.eq(route-url(route, base-path: "/docs/"), url)
// ```
//
// `[site]` `base-path` is where the published tree is served: with `base-path = "/docs/"`,
// `Hello World/index.html` is still written directly under `build.publish-dir`, with no `docs/`
// folder added. `output-url` and `route-url` use that setting by default; pass `origin: site.origin`
// for an absolute URL. Use their results directly: adding `/docs/` again duplicates the prefix.
// A site-root URL written by hand must include the prefix itself. Tola does not rewrite link strings.
//
// Routes and outputs are decoded text: `%` and `#` are literal filename characters. Browser URLs
// encode them as `%25` and `%23`; a literal `%20` in a filename becomes `%2520`. Decode an encoded,
// unmounted URL path with `decode-url-path` exactly once before using it as a route. URL queries
// and fragments are separate suffixes: append `#intro` to the completed URL for a section link;
// `#` inside a decoded route instead names a character in the filename.
//
// Use `asset-url` for an unmounted URL declared under `[assets]`, such as `/assets/logo.png`.
// This input follows URL path syntax: escape literal `%` and `#` as `%25` and `%23`, and pass no
// query or fragment. It checks the declaration, keeps its exact filename, applies `base-path`, and
// adds `?h=<asset hash>` when cache-busting is on. `output-url` instead accepts any output path
// without checking that it exists, including files a `generate-outputs` hook writes through
// `TOLA_HOOK_OUTPUT_DIR`. Document exports, native `asset()` files, and hook outputs all live under
// `build.publish-dir`; none includes the deployment prefix.

/// Pure function: turn text into a slug, the string you can put in a URL or file name unchanged.
///
/// A slug keeps letters, marks, numbers, and the punctuation that is safe in URLs and filenames:
/// `slugify("Deep Notes")` gives `"deep-notes"`. Every other character separates words, and the
/// result is NFC normalized.
///
/// Constraints: text with nothing left to slug is an error; `mode`, `case`, and `separator` accept
/// only their listed values; under `mode: "ascii"` the readings follow `language`, so only a
/// primary `ja` subtag reads Han as Japanese — every other tag, `en` included, reads it in Chinese.
///
/// Example - slug a title with the defaults, then with options:
///
/// ```typst
/// #import "@tola/address:0.0.0": slugify
/// #assert.eq(slugify("Deep Notes"), "deep-notes")
/// #assert.eq(slugify("Deep Notes", case: "upper", separator: "_"), "DEEP_NOTES")
/// ```
///
/// Example - transcribe Han text and accents to ASCII; a Han word is read as a word, so a character
/// with several readings follows the word it is in:
///
/// ```typst
/// #import "@tola/address:0.0.0": slugify
/// #assert.eq(slugify("北京 Café", mode: "ascii"), "bei-jing-cafe")
/// #assert.eq(slugify("重庆", mode: "ascii"), "chong-qing")
/// ```
/// Related: join-route
#import "@tola/host:0.0.0": slugify

/// Pure function: join a `segments` array into a decoded site-root route.
///
/// Each segment is kept exactly as spelled, so slug it first when you want slugs, other casing, or
/// transcription. A source's `route-segments` are only the default the file layout gives; a
/// document's own output decides where it is reachable.
///
/// Constraints: the result always starts and ends with `/`, and no segments give the site root `/`;
/// a segment that is empty, `.`, `..`, or that holds `/`, `\`, or a character no published filename
/// can carry is an error.
///
/// Example - keep the segments as spelled, then slug each one to publish them:
///
/// ```typ
/// #import "@tola/address:0.0.0": join-route, output-of, slugify
/// #let route = join-route(("posts", "Deep Notes").map(slugify))
/// #assert.eq(output-of(route), "posts/deep-notes/index.html")
/// #document(output-of(route), title: [Deep Notes])[Body]
/// ```
///
/// Example - read the default route the file layout gives every source:
///
/// ```typst site
/// #import "@tola/address:0.0.0": join-route
/// #import "@tola/source:0.0.0": all-sources
/// #let routes = all-sources().map(source => join-route(source.route-segments))
/// #assert.eq(routes, (
///   "/",
///   "/guide/",
///   "/guide/install/",
///   "/guide/deploy/",
///   "/guide/advanced/plugins/",
///   "/notes/first-post/",
///   "/notes/second-post/",
/// ))
/// ```
/// Related: slugify, output-of, @tola/source
#import "@tola/host:0.0.0": join-route

/// Pure function: convert a decoded site-root route to its output file path.
///
/// A route is a logical address within the site, without `base-path` or `origin`; `%` and `#`
/// are literal filename characters. A trailing `/` names a directory, published as its
/// `index.html`, so `/` is `index.html` and `/guide/` is `guide/index.html`; a route without a
/// trailing `/` names a file exactly as spelled, so `/guide` is `guide` and `/manual.pdf` is
/// `manual.pdf`.
///
/// Constraints: a route is taken as spelled, so decode a percent-encoded URL path with
/// `decode-url-path` first — exactly once.
///
/// Example - name the output file for a route:
///
/// ```typst
/// #import "@tola/address:0.0.0": output-of
/// #assert.eq(output-of("/"), "index.html")
/// #assert.eq(output-of("/guide/"), "guide/index.html")
/// #assert.eq(output-of("/manual.pdf"), "manual.pdf")
/// ```
///
/// Example - keep a literal `%` or `#` as a filename character:
///
/// ```typst
/// #import "@tola/address:0.0.0": output-of
/// #assert.eq(output-of("/100%/"), "100%/index.html")
/// #assert.eq(output-of("/a#b.html"), "a#b.html")
/// ```
/// Related: join-route, decode-url-path
#import "@tola/host:0.0.0": output-of

/// Pure function: convert an output file path to its canonical decoded site-root route.
///
/// An exact `index.html` filename becomes the directory address that contains it, so `index.html`
/// is `/` and `guide/index.html` is `/guide/`. Every other filename keeps its path under `/`, so
/// `404.html` is `/404.html` and `about` is `/about`.
///
/// The result has no deployment prefix, origin, or percent-encoding.
/// `output-of(route-of(output))` recovers the output; converting `/guide/index.html`
/// to an output and back gives its canonical route `/guide/`.
///
/// Example - map output files back to routes:
///
/// ```typst
/// #import "@tola/address:0.0.0": route-of
/// #assert.eq(route-of("index.html"), "/")
/// #assert.eq(route-of("guide/index.html"), "/guide/")
/// #assert.eq(route-of("404.html"), "/404.html")
/// ```
/// Related: output-of
#import "@tola/host:0.0.0": route-of

/// Reads site configuration (`[site]` `base-path`): the browser URL an output file is served from.
///
/// Pass a decoded output path under `build.publish-dir`, with no leading `/` or deployment
/// prefix, such as `guide/index.html`: the document's own output or the result of `output-of`.
/// `base-path` is the deployment path below the host root and defaults to this site's
/// `site.base-path`, so the result is host-relative and mounted for the site being built.
/// Pass `base-path: "/"` for a root deployment, or another path to override the site's own. Pass
/// `origin` to get an absolute URL; `origin: site.origin` uses the site's own origin.
///
/// Exact `index.html` filenames use their directory URL: `guide/index.html` becomes `/guide/`.
/// `asset-url` instead keeps a declaration's exact filename, including `index.html`.
///
/// Constraints: `base-path` must start and end with `/`, and any escapes in it are decoded once;
/// `origin` is an absolute `http`/`https` origin and cannot include a deployment path, credentials,
/// query, or fragment; output names are encoded once, so a literal `%20` becomes `%2520`.
///
/// Any output path resolves here, including one a hook produces: publication is not checked, because the
/// address is a name, not a promise that the file exists. A URL a `[assets]` declaration spells is
/// not an output path — `asset-url` resolves those, and adds their byte identity.
///
/// Keep the document output unprefixed, and use the returned URL directly in a link. With
/// `site.base-path = "/docs/"`, this page links to `/docs/guide/` while the output stays
/// `guide/index.html`. Final reference checks verify that the linked output exists.
///
/// Example - publish a page and link it from another document:
///
/// ```typst
/// #import "@tola/address:0.0.0": output-url
/// #let guide-output = "guide/index.html"
/// #document("index.html", title: [Home])[
///   #link(output-url(guide-output))[Read the guide]
/// ]
/// #document(guide-output, title: [Guide])[
///   #title()
///   Start here.
/// ]
/// ```
///
/// Example - mount an output under a base path and an origin:
///
/// ```typst
/// #import "@tola/address:0.0.0": output-url
/// #assert.eq(output-url("guide/index.html"), "/guide/")
/// #assert.eq(output-url("Hello World/index.html", base-path: "/docs/"), "/docs/Hello%20World/")
/// #assert.eq(
///   output-url("guide/index.html", base-path: "/docs/", origin: "https://example.test"),
///   "https://example.test/docs/guide/",
/// )
/// ```
///
/// Example - encode a literal `%20` in an output name exactly once:
///
/// ```typst
/// #import "@tola/address:0.0.0": output-url
/// #assert.eq(output-url("100%20.html"), "/100%2520.html")
/// ```
///
/// Example - link a hook's own output, which no `assets` declaration names:
///
/// ```typst
/// #import "@tola/address:0.0.0": output-url
/// #assert.eq(output-url("assets/pagefind-search/pagefind.js"), "/assets/pagefind-search/pagefind.js")
/// ```
/// Related: output-of, route-url, asset-url, @tola/site
/// - output (string): the decoded file path under `build.publish-dir`, without a leading `/` or deployment prefix.
/// - base-path (auto | string): the deployment path; `auto` takes the site's own `base-path`.
/// - origin (none | string): the absolute origin the result is written under.
/// -> string
#let output-url(output, base-path: auto, origin: none) = {
  import "@tola/host:0.0.0": output-url as resolve

  if base-path == auto {
    import "@tola/site:0.0.0": site
    resolve(output, base-path: site.base-path, origin: origin)
  } else {
    resolve(output, base-path: base-path, origin: origin)
  }
}

/// The browser URL of a decoded site-root route, using the canonical URL of its output.
///
/// This is `output-url(output-of(route), base-path: base-path, origin: origin)`: a trailing `/`
/// names an index output, and an explicit `/guide/index.html` also becomes `/guide/`.
/// A file route such as `/manual.pdf` keeps its filename. This does not preserve every route's
/// original spelling; it chooses the same canonical address as `output-url`.
///
/// Pass decoded text without the deployment prefix: Unicode, literal `%` and `#` are filename
/// characters. Decode an encoded URL path with `decode-url-path` exactly once before passing it.
/// The deployment prefix is added once; `base-path: auto` uses `site.base-path`, and `origin`
/// follows `output-url`.
///
/// Example - canonicalize a route, then mount and encode it:
///
/// ```typst
/// #import "@tola/address:0.0.0": route-url, decode-url-path
/// #assert.eq(route-url("/guide/index.html", base-path: "/"), "/guide/")
/// #assert.eq(route-url("/manual.pdf", base-path: "/"), "/manual.pdf")
/// #assert.eq(route-url("/中文/100%/", base-path: "/docs/"), "/docs/%E4%B8%AD%E6%96%87/100%25/")
/// #assert.eq(
///   route-url(decode-url-path("/caf%C3%A9/"), base-path: "/docs/", origin: "https://example.test"),
///   "https://example.test/docs/caf%C3%A9/",
/// )
/// ```
///
/// Related: output-url, output-of, decode-url-path
/// - route (string): the decoded site-root route to convert.
/// - base-path (auto | string): the deployment path; `auto` takes the site's own `base-path`.
/// - origin (none | string): the absolute origin the result is written under.
/// -> string
#let route-url(route, base-path: auto, origin: none) = output-url(
  output-of(route),
  base-path: base-path,
  origin: origin,
)

/// Reads site configuration (`[assets]`, `[site]` `base-path`): the browser URL a declared asset URL serves at.
///
/// Pass the unmounted site-root URL the declaration spells, without `site.base-path`:
///
/// - `assets.files` - the entry's own `url`, such as `"/styles/base.css"`.
/// - `assets.trees` - the tree's `url-prefix` followed by the member's path inside the tree:
///   `url-prefix = "/assets"` with `images/logo.svg` in that tree gives
///   `asset-url("/assets/images/logo.svg")`.
///
/// The argument follows URL path syntax and is percent-decoded once. Unicode and interior spaces
/// may appear directly or encoded; a literal `%` or `#` in a filename must be written `%25` or
/// `%23`. Queries and fragments are not accepted. Do not pass the decoded result of another call.
/// A build resolves the declaration's exact filename, including `index.html`, and appends
/// `?h=<identity>` when `assets.cache-busting` is on, so changed bytes become a changed address
/// under an unchanged name.
///
/// Constraints: the returned URL starts with `/` and includes `site.base-path`, so use it directly
/// from any document — mounting it again duplicates it; calling it for a URL that no `assets`
/// declaration publishes is an error at the call site, even when something else publishes that
/// path, such as a `resize-image` derivative.
///
/// How the two differ:
///
/// | | `output-url(output)` | `asset-url(declared URL)` |
/// | --- | --- | --- |
/// | Input | a decoded output file path under `build.publish-dir`, no leading `/` (`assets/logo.png`) | the site-root URL a declaration spells, leading `/` included (`/assets/logo.png`); URL syntax with literal `%` and `#` escaped |
/// | Publication | not checked — a hook's future output resolves too | the `[assets]` declaration must publish the URL, even when something else writes that path |
/// | Result | the mounted canonical URL; `assets/index.html` uses `/assets/` | the mounted exact file URL; `/assets/index.html` stays `/assets/index.html`, plus `?h=<identity>` when `assets.cache-busting` is on |
///
/// An editor source check resolves the mounted address without the identity, because it does not
/// render asset bytes. `tola check` prepares the complete outputs, including asset identities,
/// without publishing them.
///
/// A hook's own output is such a path: pages link it with `output-url("assets/…")`, and the two
/// calls stay apart — pass a declaration's spelling here, never another function's result.
///
/// Example - resolve a file the site publishes under `/assets`:
///
/// ```typst site
/// #import "@tola/address:0.0.0": asset-url
/// #document("index.html")[
///   #html.img(src: asset-url("/assets/logo.png"), alt: "Example logo")
/// ]
/// ```
/// Related: output-url, @tola/site, @tola/image
#import "@tola/host:0.0.0": asset-url

/// Pure function: decode a percent-encoded site-root URL path exactly once.
///
/// Every escape becomes the byte it names, so `/caf%C3%A9/` is `/café/` and `/Hello%20World/` is
/// `/Hello World/`. Decode here, once.
///
/// Constraints: pass a path and nothing else — an origin, query string, or fragment raises an
/// error, and so does an escaped `/` or `\`, because only an unescaped `/` starts a hierarchy
/// level. The decoded path must also satisfy the portable output-name rules; this is not a
/// general URL decoder and does not check that the path is published.
///
/// For metadata, `@tola/schema`'s `url-path` applies these rules while collecting field errors.
/// Its parsed value is already decoded and can be passed directly to `output-of` or `route-url`.
///
/// Example - decode a percent-encoded path exactly once:
///
/// ```typst
/// #import "@tola/address:0.0.0": decode-url-path
/// #assert.eq(decode-url-path("/caf%C3%A9/"), "/café/")
/// #assert.eq(decode-url-path("/Hello%20World/"), "/Hello World/")
/// #assert.eq(decode-url-path("/plain/"), "/plain/")
/// ```
///
/// Example - decode a path once, then name the output file:
///
/// ```typst
/// #import "@tola/address:0.0.0": decode-url-path, output-of
/// #assert.eq(decode-url-path("/100%25/"), "/100%/")
/// #assert.eq(output-of(decode-url-path("/100%25/")), "100%/index.html")
/// ```
/// Related: output-of, @tola/schema
#import "@tola/host:0.0.0": decode-url-path
