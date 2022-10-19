<script lang="ts">
  import * as pdfjsLib from "pdfjs-dist";
  import pdfjsWorker from "pdfjs-dist/build/pdf.worker.js?worker";
  import loadPage from "../pdf-table-extractor";
  // import pdfUrl from "../../samples/grater-sample.pdf";
  // import pdfUrl from "../../samples/Notenspiegel_(Alles)_12102022_0010_4713409_20221012001001.pdf";
  import pdfUrl from "../../samples/Kontoauszug_12102022_0010_4713410_20221012001019.pdf";
  import * as grater from "../../pkg/grater";

  let result = "";
  let modules = [];

  async function showPdf() {
    const pdf = await pdfjsLib.getDocument({
      url: pdfUrl,
      worker: pdfjsLib.PDFWorker.fromPort({ port: new pdfjsWorker() }),
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

    modules = grater.extract_modules(preparedData);
    result = grater.calculate_best_grade(modules);
  }
  console.log("Page loaded");

  showPdf();
</script>

<pre>{result}</pre>
<table>
  {#each modules as module}
    <tr>
      <td>{module.label}</td>
      <td>{module.credits}</td>
      <td
        >{module.grade.Numeric
          ? module.grade.Numeric.toFixed(2)
          : module.grade}</td
      >
      <td>{module.weight_modifier !== 1 ? module.weight_modifier : ""}</td>
    </tr>
  {/each}
</table>

<style>
  table,
  td {
    border: 1px solid;
  }
</style>
