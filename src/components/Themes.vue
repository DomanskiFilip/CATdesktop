<template>
  <section id="themes">  
    <h2>Themes:</h2>
    <section>
    <button id="light-1"></button>
    <button id="light-2"></button>
    <button id="light-3"></button>
    <button id="dark-1"></button>
    <button id="dark-2"></button>
    <button id="dark-3"></button>
    <button id="dark-4"></button>
    </section>
  </section>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core'

interface Theme {
  [key: string]: string;
}

const themeMappings: Record<string, Theme> = {
  "light-1": {
    "--color-main": "var(--color-light)",
    "--color-border": "var(--color-dark)",
    "--color-text": "var(--color-dark)",
    "--color-theme": "var(--color-whale)",
    "--color-action": "var(--color-theme)",
    "--color-shadow": "var(--color-shadow-light)"
  },
  "light-2": {
    "--color-main": "var(--color-light)",
    "--color-border": "var(--color-dark)",
    "--color-text": "var(--color-dark)",
    "--color-theme": "var(--color-orange)",
    "--color-action": "var(--color-theme)",
    "--color-shadow": "var(--color-shadow-light)"
  },
  "light-3": {
    "--color-main": "var(--color-light)",
    "--color-border": "var(--color-dark)",
    "--color-text": "var(--color-dark)",
    "--color-theme": "var(--color-rasberry)",
    "--color-action": "var(--color-theme)",
    "--color-shadow": "var(--color-shadow-light)"
  },
  "dark-1": {
    "--color-main": "var(--color-dark)",
    "--color-border": "var(--color-light)",
    "--color-text": "var(--color-light)",
    "--color-theme": "var(--color-whale)",
    "--color-action": "var(--color-theme)",
    "--color-shadow": "var(--color-shadow-dark)"
  },
  "dark-2": {
   "--color-main": "var(--color-dark)",
    "--color-border": "var(--color-light)",
    "--color-text": "var(--color-light)",
    "--color-theme": "var(--color-orange)",
    "--color-action": "var(--color-theme)",
    "--color-shadow": "var(--color-shadow-dark)"
  },
  "dark-3": {
   "--color-main": "var(--color-dark)",
    "--color-border": "var(--color-light)",
    "--color-text": "var(--color-light)",
    "--color-theme": "var(--color-rasberry)",
    "--color-action": "var(--color-theme)",
    "--color-shadow": "var(--color-shadow-dark)"
  },
  "dark-4": {
   "--color-main": "var(--color-dark)",
    "--color-border": "var(--color-light)",
    "--color-text": "var(--color-light)",
    "--color-theme": "var(--color-lemon)",
    "--color-action": "var(--color-theme)",
    "--color-shadow": "var(--color-shadow-dark)"
  }
};

async function applyTheme(id: string) {
  const theme = themeMappings[id];
  for (const [property, value] of Object.entries(theme)) {
    document.documentElement.style.setProperty(property, value);
  }
  try {
    await invoke('save_theme', { theme: id });
  } catch (error) {
    console.error('Failed to save theme:', error);
  }
}

onMounted(async () => {
  try {
    const savedTheme = await invoke('load_theme') as string;
    if (savedTheme && savedTheme.trim() !== '') {
      applyTheme(savedTheme);
    }
  } catch (error) {
    console.error('Failed to load theme:', error);
  }

  // Add click listeners
  Object.keys(themeMappings).forEach((id) => {
    const button = document.getElementById(id);
    if (button) {
      button.addEventListener("click", () => applyTheme(id));
    }
  });
});
</script>

<style scoped>
#themes{
  margin: 0.5rem;
}

h2 {
  color: var(--color-text);
  font-size: 1.2rem;
}

button{
  width: 50px;
  height: 50px;
  border: none;
  border-radius: 50%;
  margin: 5px;
  cursor: pointer;
}

button:hover {
  box-shadow: 0 0 10px var(--color-theme);
}

#light-1 {
  background: conic-gradient(from 225deg, var(--color-light) 0deg 180deg, var(--color-whale) 180deg 360deg);
}

#light-2 {
  background: conic-gradient(from 225deg, var(--color-light) 0deg 180deg, var(--color-orange) 180deg 360deg);
}

#light-3 {
  background: conic-gradient(from 225deg, var(--color-light) 0deg 180deg, var(--color-rasberry) 180deg 360deg);
}

#dark-1 {
  background: conic-gradient(from 225deg, var(--color-dark) 0deg 180deg, var(--color-whale) 180deg 360deg);
}

#dark-2 {
  background: conic-gradient(from 225deg, var(--color-dark) 0deg 180deg, var(--color-orange) 180deg 360deg);
}

#dark-3 {
  background: conic-gradient(from 225deg, var(--color-dark) 0deg 180deg, var(--color-rasberry) 180deg 360deg);
}

#dark-4 {
  background: conic-gradient(from 225deg, var(--color-dark) 0deg 180deg, var(--color-lemon) 180deg 360deg);
}

</style>
