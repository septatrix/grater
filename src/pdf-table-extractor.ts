import { OPS as pdfjsOps, Util } from "pdfjs-dist";
import type { PDFPageProxy } from "pdfjs-dist";
import type { TextItem } from "pdfjs-dist/types/src/display/api";

async function loadPage(page: PDFPageProxy) {
  // Get rectangle first

  let line_max_width = 2;

  let edges = await extractLines(page, line_max_width);

  // merge rectangle to verticle lines and horizon lines
  // TODO is the sort necessary?
  let verticalLines = edges.filter((e) => e.width < line_max_width);
  verticalLines.sort((a, b) => a.x - b.x || a.y - b.y);
  let horizontalLines = edges.filter((e) => e.height < line_max_width);
  horizontalLines.sort((a, b) => a.y - b.y || a.x - b.x);

  // get verticle lines
  function lines_add_verticle(lines, top, bottom) {
    for (let line of lines) {
      if (top > line.bottom || bottom < line.top) {
        continue;
      }

      // extend existing line
      line.top = Math.min(line.top, top);
      line.bottom = Math.max(line.bottom, bottom);
      return lines;
    }

    lines.push({ top: top, bottom: bottom });
    return lines;
  }

  let current_x = null;
  let current_y = null;
  let lines = [];
  let verticalRulers = [];
  let current_height = 0;
  for (let edge of verticalLines) {
    // new verticle lines
    if (null === current_x || edge.x - current_x > line_max_width) {
      if (current_height > line_max_width) {
        lines = lines_add_verticle(
          lines,
          current_y,
          current_y + current_height
        );
      }
      if (null !== current_x && lines.length) {
        verticalRulers.push({ x: current_x, lines: lines });
      }
      current_x = edge.x;
      current_y = edge.y;
      current_height = 0;
      lines = [];
    }

    if (Math.abs(current_y + current_height - edge.y) < 5) {
      current_height = edge.height + edge.y - current_y;
    } else {
      if (current_height > line_max_width) {
        lines = lines_add_verticle(
          lines,
          current_y,
          current_y + current_height
        );
      }
      current_y = edge.y;
      current_height = edge.height;
    }
  }
  if (current_height > line_max_width) {
    lines = lines_add_verticle(lines, current_y, current_y + current_height);
  }

  // no data
  if (current_x === null || lines.length == 0) {
    return [];
  }
  verticalRulers.push({ x: current_x, lines: lines });
  globalThis.temp1 = verticalRulers;

  // Get horizon lines
  function lines_add_horizon(lines, left, right) {
    for (let line of lines) {
      if (left > line.right || right < line.left) {
        continue;
      }

      line.left = Math.min(line.left, left);
      line.right = Math.max(line.right, right);
      return lines;
    }

    lines.push({ left: left, right: right });
    return lines;
  }

  current_x = null;
  current_y = null;
  let current_width = 0;
  let horizontalRulers = [];
  for (let edge of horizontalLines) {
    if (null === current_y || edge.y - current_y > line_max_width) {
      if (current_width > line_max_width) {
        lines = lines_add_horizon(lines, current_x, current_x + current_width);
      }
      if (null !== current_y && lines.length) {
        horizontalRulers.push({ y: current_y, lines: lines });
      }
      current_x = edge.x;
      current_y = edge.y;
      current_width = 0;
      lines = [];
    }

    if (Math.abs(current_x + current_width - edge.x) < 5) {
      current_width = edge.width + edge.x - current_x;
    } else {
      if (current_width > line_max_width) {
        lines = lines_add_horizon(lines, current_x, current_x + current_width);
      }
      current_x = edge.x;
      current_width = edge.width;
    }
  }
  if (current_width > line_max_width) {
    lines = lines_add_horizon(lines, current_x, current_x + current_width);
  }
  // no data
  if (current_y === null || lines.length == 0) {
    return [];
  }
  horizontalRulers.push({ y: current_y, lines: lines });
  globalThis.temp2 = horizontalRulers;

  // Detect continous tables
  let continuousRegions = detectContinuousRegions(
    verticalRulers,
    horizontalRulers
  ).sort();

  let regionsWithRulers = [];
  for (let region of continuousRegions) {
    let [xMin, yMin, xMax, yMax] = region;
    let vRulers = verticalRulers
      .filter(({ x }) => xMin <= x && x <= xMax)
      .map(({ x, lines }) => {
        // it is sufficient to check that one coordinate lies within the boundaries
        // the region is guaranteed to cover the whole line
        return {
          x,
          lines: lines.filter(({ top }) => yMin <= top && top <= yMax),
        };
      })
      .filter(({ lines }) => lines.length);

    let hRulers = horizontalRulers
      .filter(({ y }) => yMin - 1 <= y && y <= yMax + 1)
      .map(({ y, lines }) => {
        // it is sufficient to check that one coordinate lies within the boundaries
        // the region is guaranteed to cover the whole line
        return {
          y,
          lines: lines.filter(({ left }) => xMin <= left && left <= xMax),
        };
      })
      .filter(({ lines }) => lines.length);
    regionsWithRulers.push({ region, vRulers, hRulers });
  }

  let tables = [];
  for (let { vRulers, hRulers } of regionsWithRulers) {
    // handle merge cells
    let merges = detectMergedCells(vRulers, hRulers);

    let merge_alias = {};
    for (let id in merges) {
      for (let c = 0; c < merges[id].width; c++) {
        for (let r = 0; r < merges[id].height; r++) {
          if (r == 0 && c == 0) {
            continue;
          }
          merge_alias[
            `${merges[id].row + r}-${merges[id].col + c}`
          ] = `${merges[id].row}-${merges[id].col}`;
        }
      }
    }

    let table = [];
    for (let i = 0; i < hRulers.length - 1; i++) {
      table[i] = [];
      for (let j = 0; j < vRulers.length - 1; j++) {
        table[i][j] = "";
      }
    }

    const content = await page.getTextContent();
    for (let item of content.items as TextItem[]) {
      // a (m11)
      // Horizontal scaling. A value of 1 results in no scaling.
      // b (m12)
      // Vertical skewing.
      // c (m21)
      // Horizontal skewing.
      // d (m22)
      // Vertical scaling. A value of 1 results in no scaling.
      // e (dx)
      // Horizontal translation (moving).
      // f (dy)
      // Vertical translation (moving).
      let [a, b, c, d, x, y] = item.transform;

      let col = -1;
      for (let i = 0; i < vRulers.length - 1; i++) {
        if (x >= vRulers[i].x && x < vRulers[i + 1].x) {
          col = i;
          break;
        }
      }
      if (col == -1) {
        continue;
      }
      let row = -1;
      for (let i = 0; i < hRulers.length - 1; i++) {
        if (y >= hRulers[i].y && y < hRulers[i + 1].y) {
          row = hRulers.length - i - 2;
          break;
        }
      }
      if (row == -1) {
        continue;
      }

      if (`${row}-${col}` in merge_alias) {
        let id = merge_alias[`${row}-${col}`];
        [row, col] = id.split("-");
      }
      // TODO reintroduce table_pos to insert \n or at least " "
      // for intra-cell line breaks (see e.g. ET2)
      table[row][col] += item.str;
    }

    for (let id in merge_alias) {
      let [row, col] = id.split("-");
      table[row][col] = null;
    }

    if (table.length) {
      tables.push({
        table,
        merges,
        width: vRulers.length - 1,
        height: hRulers.length - 1,
      });
    }
  }
  return tables;
}

export default loadPage;

async function extractLines(page, line_max_width: number) {
  let edges = [];

  let current_x;
  let current_y;

  let transformMatrix = [1, 0, 0, 1, 0, 0];
  let transformStack = [];
  let lineWidth = null;

  const opList = await page.getOperatorList();
  while (opList.fnArray.length) {
    let fn = opList.fnArray.shift();
    let args = opList.argsArray.shift();
    if (fn === pdfjsOps.constructPath) {
      while (args[0].length) {
        let op = args[0].shift();
        if (op == pdfjsOps.rectangle) {
          let x = args[1].shift();
          let y = args[1].shift();
          let width = args[1].shift();
          let height = args[1].shift();
          if (Math.min(width, height) < line_max_width) {
            edges.push({
              y: y,
              x: x,
              width: width,
              height: height,
              transform: transformMatrix,
            });
          }
        } else if (op == pdfjsOps.moveTo) {
          current_x = args[1].shift();
          current_y = args[1].shift();
        } else if (op == pdfjsOps.lineTo) {
          let x = args[1].shift();
          let y = args[1].shift();

          if (lineWidth == null) {
            if (current_x == x) {
              edges.push({
                y: Math.min(y, current_y),
                x: x,
                height: Math.abs(y - current_y),
                transform: transformMatrix,
              });
            } else if (current_y == y) {
              edges.push({
                x: Math.min(x, current_x),
                y: y,
                width: Math.abs(x - current_x),
                transform: transformMatrix,
              });
            }
          } else {
            if (current_x == x) {
              edges.push({
                y: Math.min(y, current_y),
                // TODO see if this is necessary.
                // Make PDF with line width 100 and see where it is centered around
                x: x - lineWidth / 2,
                width: lineWidth,
                height: Math.abs(y - current_y),
                transform: transformMatrix,
              });
            } else if (current_y == y) {
              edges.push({
                x: Math.min(x, current_x),
                y: y - lineWidth / 2,
                height: lineWidth,
                width: Math.abs(x - current_x),
                transform: transformMatrix,
              });
            }
          }
          current_x = x;
          current_y = y;
        } else {
          // throw ('constructPath ' + op);
        }
      }
    } else if (fn === pdfjsOps.save) {
      transformStack.push(transformMatrix);
    } else if (fn === pdfjsOps.restore) {
      transformMatrix = transformStack.pop();
    } else if (fn === pdfjsOps.transform) {
      transformMatrix = Util.transform(transformMatrix, args);
    } else if (fn === pdfjsOps.setLineWidth) {
      lineWidth = args[0];
    } else {
    }
  }

  edges = edges.map((edge) => {
    let point1 = Util.applyTransform([edge.x, edge.y], edge.transform);
    let point2 = Util.applyTransform(
      [edge.x + edge.width, edge.y + edge.height],
      edge.transform
    );
    return {
      x: Math.min(point1[0], point2[0]),
      y: Math.min(point1[1], point2[1]),
      width: Math.abs(point1[0] - point2[0]),
      height: Math.abs(point1[1] - point2[1]),
    };
  });

  return edges;
}

function detectContinuousRegions(
  verticalRulers: any[],
  horizontalRulers: any[]
) {
  let continuousRegions = [];
  // TODO detect split tables in both directions
  // by starting with rectangles and expanding them!
  for (let v of verticalRulers) {
    for (let { top, bottom } of v.lines) {
      let containingRegion = null;
      for (let region of continuousRegions) {
        // TODO symmetric uncertainty, instead of only y+1
        // TODO create intersect functions which accepts object arguments
        // TODO create union/boundingBox function
        if (Util.intersect(region, [v.x, top, v.x + 1, bottom])) {
          region[0] = Math.min(region[0], v.x);
          region[1] = Math.min(region[1], top);
          region[2] = Math.max(region[2], v.x);
          region[3] = Math.max(region[3], bottom);
          containingRegion = region;
          break;
        }
      }
      if (!containingRegion) {
        containingRegion = [v.x, top, v.x + 1, bottom];
        continuousRegions.push(containingRegion);
      }
      for (let h of horizontalRulers) {
        for (let { left, right } of h.lines) {
          if (Util.intersect(containingRegion, [left, h.y, right, h.y + 1])) {
            containingRegion[0] = Math.min(containingRegion[0], left);
            containingRegion[1] = Math.min(containingRegion[1], h.y);
            containingRegion[2] = Math.max(containingRegion[2], right);
            containingRegion[3] = Math.max(containingRegion[3], h.y);
          }
        }
      }
    }
  }
  return continuousRegions;
}

function detectMergedCells(verticalRulers: any[], horizontalRulers: any[]) {
  function search_index(v, list) {
    for (let i = 0; i < list.length; i++) {
      if (Math.abs(list[i] - v) < 5) {
        return i;
      }
    }
    return -1;
  }

  // TODO check if we need sorting? See y_list
  let x_list = verticalRulers.map((a) => a.x);

  let verticle_merges = {};
  // skip the 1st lines and final lines
  for (let r = 0; r < horizontalRulers.length - 2; r++) {
    let hor = horizontalRulers[0 + horizontalRulers.length - r - 2];
    let col = search_index(hor.lines[0].left, x_list);
    if (col != 0) {
      for (let c = 0; c < col; c++) {
        verticle_merges[`${r}-${c}`] = {
          row: r,
          col: c,
          width: 1,
          height: 2,
        };
      }
    }
    for (let line of hor.lines) {
      let left_col = search_index(line.left, x_list);
      let right_col = search_index(line.right, x_list);
      if (left_col != col) {
        for (let c = col; c < left_col; c++) {
          verticle_merges[`${r}-${c}`] = {
            row: r,
            col: c,
            width: 1,
            height: 2,
          };
        }
      }
      col = right_col;
    }
    if (col != verticalRulers.length - 1) {
      for (let c = col; c < verticalRulers.length - 1; c++) {
        verticle_merges[`${r}-${c}`] = {
          row: r,
          col: c,
          width: 1,
          height: 2,
        };
      }
    }
  }

  while (true) {
    let merged = false;
    for (let r_c in verticle_merges) {
      let m = verticle_merges[r_c];
      let final_id = [m.row + m.height - 1, m.col + m.width - 1].join("-");
      if ("undefined" !== typeof verticle_merges[final_id]) {
        verticle_merges[r_c].height += verticle_merges[final_id].height - 1;
        delete verticle_merges[final_id];
        merged = true;
        break;
      }
    }
    if (!merged) {
      break;
    }
  }

  // TODO check if we can get rid of the sorting?
  let y_list = horizontalRulers.map((a) => a.y).sort((a, b) => b - a);

  let horizon_merges = {};
  for (let [col, ruler] of verticalRulers.slice(1, -1).entries()) {
    let currentRow = 0;
    // TODO try to eliminate need for reverse()
    for (let line of ruler.lines.reverse()) {
      let startRow = search_index(line.bottom, y_list);
      for (; currentRow < startRow; currentRow++) {
        horizon_merges[`${currentRow}-${col}`] = {
          row: currentRow,
          col,
          width: 2,
          height: 1,
        };
      }
      let endRow = search_index(line.top, y_list);
      currentRow = endRow === -1 ? y_list.length : endRow;
    }
    if (currentRow != horizontalRulers.length - 1) {
      for (let r = currentRow; r < horizontalRulers.length - 1; r++) {
        horizon_merges[`${r}-${col}`] = {
          row: r,
          col,
          width: 2,
          height: 1,
        };
      }
    }
  }

  while (true) {
    let merged = false;
    for (let r_c in horizon_merges) {
      let m = horizon_merges[r_c];
      let final_id = [m.row + m.height - 1, m.col + m.width - 1].join("-");
      if ("undefined" !== typeof horizon_merges[final_id]) {
        horizon_merges[r_c].width += horizon_merges[final_id].width - 1;
        delete horizon_merges[final_id];
        merged = true;
        break;
      }
    }
    if (!merged) {
      break;
    }
  }
  let merges = verticle_merges;
  for (let id in horizon_merges) {
    if ("undefined" !== typeof merges[id]) {
      merges[id].width = horizon_merges[id].width;
    } else {
      merges[id] = horizon_merges[id];
    }
  }
  for (let id in merges) {
    for (let c = 0; c < merges[id].width; c++) {
      for (let r = 0; r < merges[id].height; r++) {
        if (c == 0 && r == 0) {
          continue;
        }
        delete merges[[r + merges[id].row, c + merges[id].col].join("-")];
      }
    }
  }
  return merges;
}
