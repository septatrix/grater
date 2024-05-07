<script setup lang="ts">
import { reactive, ref } from "vue";

import uploadIconSvg from "bootstrap-icons/icons/file-earmark-arrow-up.svg?raw";
import { PDFWorker, getDocument } from "pdfjs-dist";
import pdfjsWorker from "pdfjs-dist/build/pdf.worker?worker";
import { calculate_best_grade, extract_modules } from "../../pkg/grater";
import loadPage from "../pdf-table-extractor";

let result = ref("");
let modulesByCategory: Record<string, any[]> = reactive({});

async function loadPdf(e: Event) {
  let fileInput = e.target as HTMLInputElement;
  let files = fileInput.files!;
  let data = await files[0].arrayBuffer();

  const pdf = await getDocument({
    data,
    worker: PDFWorker.fromPort({ port: new pdfjsWorker() }),
    isEvalSupported: false,
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
        row.map((cell) => (cell === null ? "" : (cell as string).trim()))
      )
    );
  preparedData.forEach((table) => console.table(table));
  let modules = extract_modules(preparedData);
  console.table(modules);
  for (let module of modules) {
    if (!(module.category in modulesByCategory)) {
      modulesByCategory[module.category] = [];
    }
    modulesByCategory[module.category].push({
      ...module,
      grade: module.grade === "Passed" ? "B" : module.grade.Numeric.toFixed(1),
    });
  }
}

function onSubmit() {
  let modules = Object.values(modulesByCategory)
    .flat()
    .map((module) => ({
      ...module,
      grade:
        module.grade === "B" ? "Passed" : { Numeric: parseFloat(module.grade) },
    }));
  result.value = calculate_best_grade(modules);
}
</script>

<template>
  <pre>{{ result }}</pre>

  <form @submit.prevent="onSubmit">
    <label class="w-100 mb-3">
      <span class="btn btn-secondary w-100">
        <span v-html="uploadIconSvg" /> Upload Kontoauszug
      </span>
      <input class="visually-hidden" type="file" @input="loadPdf" />
    </label>

    <button
      class="btn btn-primary mb-3 w-100"
      :disabled="!Object.values(modulesByCategory).flat().length"
    >
      Calculate
    </button>
    <fieldset
      v-for="(modules, category) in modulesByCategory"
      class="card mb-3 shadow-sm"
    >
      <div class="card-body p-2">
        <legend class="card-title h5">{{ category }}</legend>
        <div class="row g-1">
          <template v-for="mod in modules">
            <div class="col-8">
              <label class="w-100">
                <span class="visually-hidden">Modulname</span>
                <input
                  :value="mod.label"
                  @input="(e) => (mod.label = (e.target as HTMLInputElement).value)"
                  class="form-control p-1"
                />
              </label>
            </div>
            <div class="col">
              <label class="w-100">
                <span class="visually-hidden">ECTS</span>
                <input
                  v-model="mod.credits"
                  class="form-control p-1 text-center"
                />
              </label>
            </div>
            <div class="col">
              <label class="w-100">
                <span class="visually-hidden">Grade</span>
                <input
                  v-model="mod.grade"
                  class="form-control p-1 text-center"
                />
              </label>
            </div>
            <div class="col">
              <label class="w-100">
                <span class="visually-hidden">Weight</span>
                <input
                  v-model="mod.weight_modifier"
                  class="form-control p-1 text-center"
                />
              </label>
            </div>
          </template>
        </div>
      </div>
    </fieldset>
  </form>
</template>
