import { mount } from "svelte";
import "./app.css";
import "./lib/manager.css";

// The menu bar popover is its own window, loaded with ?view=menubar. Each window loads only
// its own code, so the popover opens fast.
const menubar = new URLSearchParams(location.search).get("view") === "menubar" || location.hash.includes("view=menubar");
if (menubar) document.documentElement.classList.add("menubar");

const target = document.getElementById("app")!;
if (menubar) import("./MenuBar.svelte").then((m) => mount(m.default, { target }));
else import("./App.svelte").then((m) => mount(m.default, { target }));
