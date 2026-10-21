import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import SettingsApp from "./settings/SettingsApp";
import "./styles/app.css";

// The Settings window loads index.html?view=settings (see settings_window.rs).
const params = new URLSearchParams(window.location.search);
const isSettings = params.get("view") === "settings";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isSettings ? <SettingsApp initialTab={params.get("tab")} notice={params.get("notice")} /> : <App />}
  </React.StrictMode>,
);
