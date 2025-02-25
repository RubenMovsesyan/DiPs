import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";


// let greetInputEl: HTMLInputElement | null;
// let greetMsgEl: HTMLElement | null;

let input_file_path_element: HTMLElement | null;
let output_file_path_element: HTMLElement | null;

async function get_file_path() {
  if (input_file_path_element) {
    try {
      const selected_file = await open({
        directory: false,
        multiple: false,
      });

      input_file_path_element.textContent = selected_file;
    } catch (error) {
      input_file_path_element.textContent = error.toString();
    }
  }
}

async function save_file_path() {
  if (output_file_path_element) {
    try {
      const path = await save({
        title: "Save Video File",
        filters: [{
          name: 'Video Files',
          extensions: ['avi', 'mp4', 'mov']
        }],
      });

      output_file_path_element.textContent = path;
    } catch (error) {
      output_file_path_element.textContent = error.toString();
    }
  }
}

async function run_dips() {
  if (input_file_path_element && output_file_path_element) {
    await invoke("run_dips", {
      input_path: input_file_path_element.textContent,
      output_path: output_file_path_element.textContent,
    });
  }
}

window.addEventListener("DOMContentLoaded", () => {
  input_file_path_element = document.querySelector("#input-file-path-element");
  document.querySelector("#input-picker")?.addEventListener("click", (e) => {
    e.preventDefault();
    get_file_path();
  });

  
  output_file_path_element = document.querySelector("#output-file-path-element");
  document.querySelector("#output-picker")?.addEventListener("click", (e) => {
    e.preventDefault();
    save_file_path();
  });

  document.querySelector("#dips-invoker")?.addEventListener("click", (e) => {
    e.preventDefault();
    run_dips();
  })
});
