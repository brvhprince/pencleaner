import { mount } from "svelte";
import "./app.css";
import "./lib/manager.css";
import MenuBar from "./MenuBar.svelte";

// The menu bar popover is its own window, loaded with ?view=menubar. It's small, so it's
// bundled up front; the main app loads on demand, so the popover never pays for it.
// (One lazy import only: with two behind a condition, the bundler preloaded the main app's
// CSS for both and the popover lost its styles.)
const menubar = new URLSearchParams(location.search).get("view") === "menubar" || location.hash.includes("view=menubar");
if (menubar) document.documentElement.classList.add("menubar");

const target = document.getElementById("app")!;
if (menubar) mount(MenuBar, { target });
else import("./App.svelte").then((m) => mount(m.default, { target }));
