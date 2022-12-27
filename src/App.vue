<script setup lang="ts">
import Grater from "./components/Grater.vue";
import graterUrl from "./assets/grater.svg?url";
</script>

<template>
  <main class="container">
    <div class="row">
      <div class="col-1 p-3">
        <img :src="graterUrl" alt="Grater" class="w-100" />
      </div>
      <div class="col-11 p-3">
        <h1>Grater!</h1>
        <h2>The greatest grade grater, guaranteed!</h2>
      </div>
    </div>
    <Grater />
  </main>
</template>

<style>
@import "bootstrap";

:root {
  overflow-y: scroll;
}
</style>
