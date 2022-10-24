<script lang="ts">
  import * as pdfjs from "pdfjs-dist/build/pdf";
  import pdfjsWorker from "pdfjs-dist/build/pdf.worker?worker";
  import loadPage from "../pdf-table-extractor";
  import * as grater from "../../pkg/grater";
  import type { Module } from "../../pkg/grater";
  import trashIconSvg from "bootstrap-icons/icons/trash.svg?raw";
  import uploadIconSvg from "bootstrap-icons/icons/file-earmark-arrow-up.svg?raw";

  let result = "";
  let modulesByCategory: Record<string, Module[]> = {};

  async function loadPdf(e) {
    let fileInput: HTMLInputElement = e.target;
    let files = fileInput.files;
    let data = await files[0].arrayBuffer();

    const pdf = await pdfjs.getDocument({
      data,
      worker: pdfjs.PDFWorker.fromPort({ port: new pdfjsWorker() }),
    }).promise;

    let loadPromises = [];

    // We only care for the first half of pages
    // as the second half is the same in english
    for (let i = 1; i <= pdf.numPages / 2; i++) {
      loadPromises.push(
        (async () => {
          const page = await pdf.getPage(i);
          return await loadPage(page);
        })()
      );
    }

    const rawData = await Promise.all(loadPromises);
    const preparedData = rawData
      .flat()
      .map((table) =>
        table.table.map((row) =>
          row.map((cell) => (cell === null ? "" : cell.trim()))
        )
      );
    preparedData.forEach((table) => console.table(table));
    let modules = grater.extract_modules(preparedData);
    console.table(modules);
    for (let module of modules) {
      if (!(module.category in modulesByCategory)) {
        modulesByCategory[module.category] = [];
      }
      modulesByCategory[module.category].push(module);
    }
  }

  function onSubmit() {
    let modules = Object.values(modulesByCategory).flat();
    result = grater.calculate_best_grade(modules);
  }
</script>

<pre>{result}</pre>

<form on:submit|preventDefault={onSubmit}>
  <label class="w-100 mb-3">
    <span class="btn btn-secondary w-100">
      {@html uploadIconSvg} Upload Kontoauszug
    </span>
    <input class="visually-hidden" type="file" on:input={loadPdf} />
  </label>

  <button
    class="btn btn-primary mb-3 w-100"
    disabled={!Object.values(modulesByCategory).flat().length}>Calculate</button
  >
  {#each Object.entries(modulesByCategory) as [category, modules]}
    <fieldset class="card mb-3 shadow">
      <div class="card-body">
        <legend class="card-title">{category}</legend>
        <div class="row g-2">
          {#each modules as mod}
            <div class="col-8">
              <label class="w-100">
                <span class="visually-hidden">Modulname</span>
                <input disabled value={mod.label} class="form-control" />
              </label>
            </div>
            <div class="col">
              <label class="w-100">
                <span class="visually-hidden">ECTS</span>
                <input
                  disabled
                  value={mod.credits}
                  class="form-control text-center"
                />
              </label>
            </div>
            <div class="col">
              <label class="w-100">
                <span class="visually-hidden">Grade</span>
                <input
                  disabled
                  value={mod.grade == "Passed"
                    ? "B"
                    : mod.grade.Numeric.toFixed(1)}
                  class="form-control text-center"
                />
              </label>
            </div>
            <div class="col">
              <label class="w-100">
                <span class="visually-hidden">Weight</span>
                <input
                  disabled
                  value={mod.weight_modifier}
                  class="form-control text-center"
                />
              </label>
            </div>
          {/each}
        </div>
      </div>
    </fieldset>
  {/each}
</form>
