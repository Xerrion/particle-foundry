import { mount } from "svelte";
import App from "./App.svelte";
import "./observability/init";

const target = document.getElementById("app");
if (!target) {
  throw new Error("Svelte application root was not found");
}

mount(App, {
  target,
});
