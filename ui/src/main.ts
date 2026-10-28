import { mount } from "svelte";
import "./app.css";
import "./lib/manager.css";
import App from "./App.svelte";
import MenuBar from "./MenuBar.svelte";

// The menu bar popover is its own window, loaded with ?view=menubar.
const menubar = new URLSearchParams(location.search).get("view") === "menubar" || location.hash.includes("view=menubar");
if (menubar) document.documentElement.classList.add("menubar");

export default mount(menubar ? MenuBar : App, { target: document.getElementById("app")! });
