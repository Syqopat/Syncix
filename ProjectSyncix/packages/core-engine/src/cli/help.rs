//! The help text.


pub(crate) fn print_help() {
    println!(
        r#"Syncix CLI  (version {})

  Status
    syncix status                 core and Studio connection, metrics
    syncix config                 show the settings in effect
    syncix bind                   show a place/folder mismatch
    syncix bind --studio|--disk   resolve it (which side is right)
    syncix verify                 check model/disk consistency
    syncix trash [--files]        what the reconciler removed (runs, or single files)
    syncix restore [run]          put a whole trash run back (newest by default)
    syncix restore <name> [--in path] [--class c] [--since 2h] [--dry-run] [--all]
                                  put single instances back (newest copy of each)
    syncix pull                   ask Studio to resend the tree (source of truth)
    syncix selftest               run an end-to-end scenario against real Studio

  Output
    syncix sourcemap [-o file]    generate sourcemap.json for luau-lsp
    syncix build [-o file] [target]
                                  export the tree as Roblox XML (.rbxmx)
    syncix import <file> [parent] import a .rbxmx/.rbxlx file into the tree
    syncix upload                 show what would be published (does nothing)
    syncix upload --confirm       actually publish to Roblox

  Inspect
    syncix tree [target]          show the tree
    syncix ls [target]            list a node's children
    syncix find <word>            search by name or class
    syncix props <target>         show every property of an instance

  Edit
    syncix new <class> [name] [parent]
    syncix rename <target> <new name>
    syncix rm <target> [--yes]    asks for confirmation unless --yes
    syncix mv <target> <new parent>
    syncix set <target> <property> <value>
    syncix attr <target> <name> <value>
    syncix attr <target> <name> --delete    remove an attribute
    syncix tag <target>           show CollectionService tags
    syncix tag <target> <tag>...  replace the tag list (--none clears it)

  Process
    syncix up [port]              start the core in the background
    syncix serve [port]           run the core in this terminal
    syncix down                   stop the core
    syncix init                   create syncix.toml and the sync folder

  Without a port the value from syncix.toml is used, otherwise 8080.
  If that port is taken the next one is tried and the Studio plugin finds it.
  To pin a specific port: syncix serve 25565, then type 25565 into the port
  field in the Studio plugin panel.

  Targets: full UUID, 8-char short UUID, name, or dot path
           (e.g. Workspace.Simulator.SellPad)
  Values:  5 | true | "text" | 0,0.5,-60 (Vector3) | #ff8800 (color)
           | Enum.Material.Neon
"#,
        crate::project::VERSION
    );
}
