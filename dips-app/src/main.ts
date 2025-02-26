import { invoke } from "@tauri-apps/api/core";
import { appLocalDataDir } from "@tauri-apps/api/path";
import { open, save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";

let input_file_path_element: HTMLElement | null;
let output_file_path: string | null = null;
let dips_path: string | null;

async function get_file_path() {
  if (input_file_path_element) {
    try {
      const selected_file = await open({
        directory: false,
        multiple: false,
      });

      dips_path = selected_file;

      let thumbnail_path: string = await appLocalDataDir();
      thumbnail_path = thumbnail_path.concat("/input_thumbnail.jpeg");
  
      console.log(thumbnail_path);
    
      await invoke("get_thumbnail", {
        input_path: selected_file,
        cache_path: thumbnail_path,
      });

      thumbnail_path = convertFileSrc(thumbnail_path);      

      try {
        input_file_path_element.innerHTML = `
          <img id="thumbnail" src="${thumbnail_path}" />
          <p>${selected_file}<p>
        `;
      } catch (error) {
        console.log(error);
      }

      console.log(input_file_path_element);
    } catch (error: any) {
      input_file_path_element.textContent = error.toString();
    }
  }
}

async function save_file_path() {
    try {
      const path = await save({
        title: "Save Video File",
        filters: [{
          name: 'Video Files',
          extensions: ['avi', 'mp4', 'mov']
        }],
      });

      output_file_path = path;
    } catch (error) {
      console.log(error);
      output_file_path = null;
    }
}

async function run_dips() {
  if (input_file_path_element) {
    if (output_file_path == null) {
      await save_file_path();
    }
    
    console.log(output_file_path);
    await invoke("run_dips", {
      input_path: dips_path,
      output_path: output_file_path,
    });
  }
}

window.addEventListener("DOMContentLoaded", () => {
  input_file_path_element = document.querySelector("#video-container");
  document.querySelector("#input-picker")?.addEventListener("click", (e) => {
    e.preventDefault();
    get_file_path();
  });

  document.querySelector("#dips-invoker")?.addEventListener("click", (e) => {
    e.preventDefault();
    run_dips();
  })
});
