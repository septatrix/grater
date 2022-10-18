<script lang="ts">
  import * as pdfjsLib from "pdfjs-dist";
  import pdfjsWorker from "pdfjs-dist/build/pdf.worker.js?worker";
  import loadPage from "../pdf-table-extractor";
  import samplePdf from "../../samples/grater-sample.pdf";

  let canvas: HTMLCanvasElement;
  let loadPromises = [];

  async function showPdf() {
    const pdf = await pdfjsLib.getDocument({
      url: samplePdf,
      worker: pdfjsLib.PDFWorker.fromPort({ port: new pdfjsWorker() }),
    }).promise;

    loadPromises = [];

    for (let i = 1; i <= pdf.numPages; i++) {
      loadPromises.push(
        (async () => {
          const page = await pdf.getPage(i);
          return await loadPage(page);
        })()
      );
    }

    const page = await pdf.getPage(1);
    let viewport = page.getViewport({ scale: 1 });

    // Prepare canvas using PDF page dimensions
    let context = canvas.getContext("2d");
    canvas.height = viewport.height;
    canvas.width = viewport.width;

    // Render PDF page into canvas context
    await page.render({
      canvasContext: context,
      viewport: viewport,
    }).promise;

    context.strokeStyle = "red";
    context.lineWidth = 2;
    context.setTransform(new DOMMatrix(viewport.transform));
    context.beginPath();
    for (let col of temp1) {
      for (let line of col.lines) {
        context.strokeRect(col.x, line.top, 2, 2);
        context.moveTo(col.x, line.top);
        context.lineTo(col.x, line.bottom);
      }
    }
    context.stroke();
    context.closePath();
    context.strokeStyle = "blue";
    context.beginPath();
    for (let col of temp2) {
      for (let line of col.lines) {
        context.strokeRect(line.left, col.y, 2, 2);
        context.moveTo(line.left, col.y);
        context.lineTo(line.right, col.y);
      }
    }
    context.stroke();
    context.closePath();
    globalThis.ctx = context;

    Promise.all(loadPromises).then(console.log);
  }
  console.log("Page loaded");

  showPdf();
</script>

<canvas bind:this={canvas} style="vertical-align: top;" />

{#each loadPromises as loadPromise, i}
  <div class="table-container" style="width: 45%; display: inline-block;">
    <h3>Page: {i + 1}</h3>
    {#await loadPromise}
      Loading...
    {:then tables}
      {#if !tables}
        No data found
      {:else}
        {#each tables as table}
          <p>Dimensions: {table.height} x {table.width}</p>

          <table>
            {#each table.table as row, r}
              <tr>
                {#each row as data, c}
                  {#if data != null}
                    <td
                      colspan={(table.merges[`${r}-${c}`] || {}).width}
                      rowspan={(table.merges[`${r}-${c}`] || {}).height}
                      >{data}</td
                    >
                  {/if}
                {/each}
              </tr>
            {/each}
          </table>
        {/each}
      {/if}
    {/await}
  </div>
{/each}

<style>
  table,
  td {
    border: 1px solid;
  }
  table {
    display: inline-block;
  }
</style>
