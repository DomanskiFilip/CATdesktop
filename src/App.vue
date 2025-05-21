<template>
  <div id="theme-background-element"></div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'

async function loadItems() {
  try {
    const body = await invoke<string>('fetch_lambda_endpoint')
    if (!body) throw new Error('No data returned')
    const outer = JSON.parse(body)
    const response = typeof outer.body === 'string' ? JSON.parse(outer.body) : outer.body
    const endpoint = document.getElementById('endpoint')
    if (endpoint) {
      if (response.status === 'ok') {
        endpoint.innerHTML = `Lambda connection: ${response.message}<br>`
        if (Array.isArray(response.items)) {
          response.items.forEach((item: any) => {
            endpoint.innerHTML += `id: ${item.id}, text: ${item.text}<br>`
          })
        } else {
          endpoint.innerHTML += 'No items found.'
        }
      } else {
        console.log('Lambda raw response:', body)
        endpoint.innerHTML = `Lambda error: ${response.message || 'Unknown error'}`
      }
    }
  } catch (err) {
    console.error('Failed to load items:', err)
    const endpoint = document.getElementById('endpoint')
    if (endpoint) {
      endpoint.innerHTML = 'Failed to connect to Lambda.'
    }
  }
}

window.onload = loadItems
</script>

<style scoped>

</style>
