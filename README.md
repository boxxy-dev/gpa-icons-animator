# GPA Icons Animator

> **Note:** This application is currently in **preview**, but it already works!

![GPA Icons Animator Preview](https://i.imgur.com/ZJk4U2P.png)

**GPA Icons Animator** is a native GNOME studio built with Rust, GTK 4.24 (`GtkSvg`), and Libadwaita 1.10 for creating, previewing, animating, and exporting **GTK Path Animation (`.gpa` / Grappa)** symbolic icons.

While you can tweak states and XML manually in the UI, **the app is specifically built to be driven by AI agents via its embedded MCP server** — best used with [**Boxxy**](https://github.com/boxxy-dev/boxxy) :)

## How It Works

1. **Connect Your Agent via MCP (`http://127.0.0.1:44820/mcp`):**
   - The app runs a built-in Streamable HTTP MCP server out of the box and serves the complete `.gpa` authoring specification (`GPA_SPECS.md`) plus live tools to inspect, draw, morph, and animate shapes and states in real time.
   - Click the **`MCP :44820`** pill in the top bar to copy the MCP URL or ready-to-use agent prompt in one click.
2. **Pick an Icon or Start from Scratch:**
   - Ask your agent to create a brand-new icon on the blank `16×16` canvas, or pick any of the **1,100+ bundled GNOME Icon Development Kit icons** and **GPA Examples** from the left sidebar for your agent to animate.
3. **Watch Live on the Studio Canvas:**
   - Every tool call from your agent immediately updates the live `GtkSvg` preview, state transition cards, 16×16 grid / X-Ray spine overlays, and `48px` / `24px` scale comparison.
4. **Save to Your Library or Export:**
   - Save finished icons to your custom library folders (`~/.config/gpa-icons-animator/libs/my-icons`) or export standalone `.gpa` files.

## Run

```bash
cargo run --release
```
