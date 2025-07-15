<template>
  <section id="calendar">
    <section id="calendar-container">
      <section id="calendar-header">
        <h2>{{ currentMonth }} {{ currentYear }}</h2>
        <button @click="previousMonth"><svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M480-528 296-344l-56-56 240-240 240 240-56 56-184-184Z"/></svg></button>
        <button @click="nextMonth"><svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M480-344 240-584l56-56 184 184 184-184 56 56-240 240Z"/></svg></button>
      </section>
      <section id="calendar-grid">
        <!-- Days of week -->
        <div v-for="day in daysOfWeek" :key="day" class="days-of-week header">
          {{ day }}
        </div>
        <!-- Calendar days -->
        <div v-for="date in calendarDays" :key="date.id" class="calendar-cell" :class="{ 'current-day': isToday(date), 'active': activeCell === date.id }" @click="selectDate(date)">
          {{ date.day }}
          <div class="indicators">
            <span v-if="getWeatherDescription(date.id)" class="weather-indicator" v-html="weatherIcon(getWeatherDescription(date.id))"></span>
            <span class="event-indicator" v-if="getEventsForDate(date).length > 0"></span>
          </div>
        </div>
      </section>
    </section>
    <section id="schedule-container">
      <h3> {{ currentDate.toLocaleDateString('default', { month: 'long', day: 'numeric', year: 'numeric' }) }} Schedule </h3>
      <section id="day-schedule-container">
        <span v-for="hour in Array.from({ length: 24 }, (_, i) => (i + 1) % 24)" 
          :key="hour" 
          class="hour">
          <span class="hour-wrapper">
            <span :class="{ 'event-time': hasEventAtHour(hour) }">
              {{ hour < 10 ? '0' + hour : hour }}:00
            </span>
            <button class="alarm" :class="{ 'in-the-past': isInPast(hour) || isNow(hour), 'alarm-on' : isAlarmOn(hour) }" @click="alarm(hour)" :disabled="isInPast(hour) || !hasEventAtHour(hour)" title="toggle alarm">
              <svg v-if="isAlarmOn(hour)" xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M96-528q0-88.39 35.5-162.19Q167-764 230-818l51 50q-52 43-82.5 105.5T168-528H96Zm696 0q0-73-30.5-135.5T678-769l52-51q62 53 98 128.5T864-528h-72ZM192-216v-72h48v-240q0-87 53.5-153T432-763v-53q0-20 14-34t34-14q20 0 34 14t14 34v53q85 16 138.5 82T720-528v240h48v72H192Zm288-276Zm-.21 396Q450-96 429-117.15T408-168h144q0 30-21.21 51t-51 21ZM312-288h336v-240q0-70-49-119t-119-49q-70 0-119 49t-49 119v240Z"/></svg>
              <svg v-else xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M192-216v-72h48v-240q0-87 53.5-153T432-763v-53q0-20 14-34t34-14q20 0 34 14t14 34v53q85 16 138.5 82T720-528v240h48v72H192Zm288-276Zm-.21 396Q450-96 429-117.15T408-168h144q0 30-21.21 51t-51 21ZM312-288h336v-240q0-70-49-119t-119-49q-70 0-119 49t-49 119v240Z"/></svg>
             </button>
             <button class="delete-btn" @click="deleteEvent(hour)" title="delete event" v-if="hasEventAtHour(hour) && !isInPast(hour) && !isNow(hour)">
              <svg xmlns="http://www.w3.org/2000/svg" height="18px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M280-120q-33 0-56.5-23.5T200-200v-520h-40v-80h200v-40h240v40h200v80h-40v520q0 33-23.5 56.5T680-120H280Zm400-600H280v520h400v-520ZM360-280h80v-360h-80v360Zm160 0h80v-360h-80v360ZM280-720v520-520Z"/></svg>
             </button>
             <button class="smart-features-btn" @click="openSmartFeatures(hour)" title="Smart Features" v-if="hasEventAtHour(hour) && !isInPast(hour) && !isNow(hour)">
               <svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M323-160q-11 0-20.5-5.5T288-181l-78-139h58l40 80h92v-40h-68l-40-80H188l-57-100q-2-5-3.5-10t-1.5-10q0-4 5-20l57-100h104l40-80h68v-40h-92l-40 80h-58l78-139q5-10 14.5-15.5T323-800h97q17 0 28.5 11.5T460-760v160h-60l-40 40h100v120h-88l-40-80h-92l-40 40h108l40 80h112v200q0 17-11.5 28.5T420-160h-97Zm217 0q-17 0-28.5-11.5T500-200v-200h112l40-80h108l-40-40h-92l-40 80h-88v-120h100l-40-40h-60v-160q0-17 11.5-28.5T540-800h97q11 0 20.5 5.5T672-779l78 139h-58l-40-80h-92v40h68l40 80h104l57 100q2 5 3.5 10t1.5 10q0 4-5 20l-57 100H668l-40 80h-68v40h92l40-80h58l-78 139q-5 10-14.5 15.5T637-160h-97Z"/></svg>
            </button>
             <button class="expand" @click="toggleExpand(hour)" title="expand/collapse">
              <svg v-if="!expand[hour]" xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M480-344 240-584l56-56 184 184 184-184 56 56-240 240Z"/></svg>
              <svg v-else xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M480-528 296-344l-56-56 240-240 240 240-56 56-184-184Z"/></svg>
             </button>
          </span>
          <!-- Hidden textarea for editing (shows when clicking the display div) -->
          <textarea v-show="activeEditor === `editor-${getHourKey(hour)}`" 
                   :class="{ 'text-editor': true, 'in-the-past': isInPast(hour), 'has-event': hasEventAtHour(hour), 'expand': expand[hour] }" 
                   :id="`editor-${getHourKey(hour)}`" 
                   v-model="hourInputs[getHourKey(hour)]" 
                   @input="updateEventDescription($event, hour)" 
                   @blur="handleBlur(hour)" 
                   :disabled="isInPast(hour) || isNow(hour)" 
                   :placeholder="isInPast(hour) || isNow(hour) ? '' : 'Add event...'">
          </textarea>
          <!-- Display div with linked text (shows when not editing) -->
          <div v-show="activeEditor !== `editor-${getHourKey(hour)}`" :class="{ 'link-display': true, 'in-the-past': isInPast(hour), 'has-event': hasEventAtHour(hour), 'expand': expand[hour] }" v-html="formatLinkedText(hourInputs[getHourKey(hour)] || getEventDescription(hour))" @click="startEditing(hour)"></div>
        </span>
      </section>
    </section>
    <smartFeatures v-if="showSmartFeatures" :event="smartFeaturesEvent" @close="closeSmartFeatures"/>
   </section>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import linkifyStr from 'linkify-string'
import smartFeatures from './smartFeatures.vue'

// interface for calendar days
interface CalendarDay {
  id: string;
  day: number | string;
  date: Date | null;
}

interface CalendarEvent {
  id: string;
  user_id: string;
  description: string;
  time: string;
  alarm: boolean;
  synced: boolean;
  deleted: boolean;
}

// interface for daily weather data
interface DailyWeather {
  date: string;
  weather: string;
  temperature_2m_max: number;
  wind_speed_10m_max: number;
}

// constant for days of the week
const daysOfWeek = ['M', 'T', 'W', 'T', 'F', 'S', 'S']
const now = new Date()
const refreshInterval = 60000 // 1 minute in milliseconds
const cleanupInterval = 3600000 // 1 hour in milliseconds

// state managment variables
const currentDate = ref(new Date())
const currentMonth = ref(currentDate.value.toLocaleString('default', { month: 'long' }))
const currentYear = ref(currentDate.value.getFullYear())
const events = ref<CalendarEvent[]>([])
const activeCell = ref<string | null>(null)
const calendarDays = ref<CalendarDay[]>([])
const expand = ref<Record<number, boolean>>({})
const hourInputs = ref<Record<string, string>>({});
const activeEditor = ref<string | null>(null)
const weather = ref<Record<string, DailyWeather> | 'no data'>('no data')
const showSmartFeatures = ref(false)
const smartFeaturesEvent = ref<CalendarEvent | null>(null)

// == Utility functions == //
// utility function -> check if its current day
const isToday = (date: CalendarDay) => {
  const today = new Date()
  return date.date && date.date.getDate() === today.getDate() &&
         date.date.getMonth() === today.getMonth() &&
         date.date.getFullYear() === today.getFullYear()
}

// utility function -> check if hour is in the past
const isInPast = (hour: number): boolean => {
  const currently = new Date()
  // If it's a future date, nothing is in the past
  if (currentDate.value.getTime() > currently.setHours(23,59,59,999)) {
    return false
  }
  
  // If it's a past date, everything is in the past
  if (currentDate.value.getTime() < currently.setHours(0,0,0,0)) {
    return true
  }
  
  // If it's today, compare hours
  if (hour === 0) {
    return false
  }

  // If it's today, compare hours
  if (currentDate.value.getDate() === currently.getDate() && 
      currentDate.value.getMonth() === currently.getMonth() && 
      currentDate.value.getFullYear() === currently.getFullYear()) {
    const currentHour = new Date().getHours()
    return hour < currentHour
  }
  
  return false
}

// utility function -> check if hour is the current hour
const isNow = (hour: number): boolean => {
  if (currentDate.value.getDate() === now.getDate() && 
      currentDate.value.getMonth() === now.getMonth() && 
      currentDate.value.getFullYear() === now.getFullYear()) {
    const currentHour = new Date().getHours()
    return hour == currentHour
  }

  return false
}

// utility function -> toggle expand state for a specific hour
const toggleExpand = (hour: number) => {
  expand.value[hour] = !expand.value[hour]
}

// helper function -> find an event at a specific hour
const findEventAtHour = (hour: number, date: Date = currentDate.value): CalendarEvent | undefined => {
  return events.value.find(event => {
    const eventDate = new Date(event.time)
    const eventHour = eventDate.getHours()
    return eventHour === hour &&
           eventDate.getDate() === date.getDate() &&
           eventDate.getMonth() === date.getMonth() &&
           eventDate.getFullYear() === date.getFullYear()
  })
}

//  utility function -> Get event description for a specific hour
const getEventDescription = (hour: number) => {
  const existingEvent = findEventAtHour(hour)
  return existingEvent ? existingEvent.description : ''
}

// utility function -> Check if alarm is on for a specific hour
const isAlarmOn = (hour: number): boolean => {
  const existingEvent = findEventAtHour(hour)
  return existingEvent ? existingEvent.alarm : false
}

// utility function -> Check if there is an event at a specific hour for frontend display
const hasEventAtHour = (hour: number) => {
  return !!findEventAtHour(hour)
}

// utility function -> Get events for specific date
const getEventsForDate = (date: CalendarDay) => {
  if (!date.date) return []
  return events.value.filter(event => {
    const eventDate = new Date(event.time)
    return eventDate.getDate() === date.date?.getDate() &&
           eventDate.getMonth() === date.date?.getMonth() &&
           eventDate.getFullYear() === date.date?.getFullYear()
  })
}

// utility function -> format text with linkify
const formatLinkedText = (text: string): string => {
  if (!text) return '';
  
  // Use linkify-string to convert URLs to HTML links
  return linkifyStr(text, {
    defaultProtocol: 'https',
    target: '_blank',
    rel: 'noopener noreferrer'
  });
}

// Utility function to get weather description for a date string
const getWeatherDescription = (dateStr: string | Date): string => {
  if (weather.value === 'no data') return '';

  // If dateStr is a Date object, format it
  let key: string;
  if (typeof dateStr === 'object' && dateStr instanceof Date) {
    key = `${dateStr.getFullYear()}-${String(dateStr.getMonth() + 1).padStart(2, '0')}-${String(dateStr.getDate()).padStart(2, '0')}`;
  } else {
    // If dateStr is already a string, try to parse and format it
    const parts = dateStr.split('-');
    if (parts.length === 3) {
      key = `${parts[0]}-${String(parts[1]).padStart(2, '0')}-${String(parts[2]).padStart(2, '0')}`;
    } else {
      key = dateStr;
    }
  }

  return weather.value[key]?.weather || '';
};

const weatherIcon = (desc: string) => {
  if (!desc) return '';
  const lowerDesc = desc.toLowerCase();
  if (lowerDesc.includes('clear sky')) {
    // Sun SVG
    return `<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M440-760v-160h80v160h-80Zm266 110-55-55 112-115 56 57-113 113Zm54 210v-80h160v80H760ZM440-40v-160h80v160h-80ZM254-652 140-763l57-56 113 113-56 54Zm508 512L651-255l54-54 114 110-57 59ZM40-440v-80h160v80H40Zm157 300-56-57 112-112 29 27 29 28-114 114Zm283-100q-100 0-170-70t-70-170q0-100 70-170t170-70q100 0 170 70t70 170q0 100-70 170t-170 70Zm0-80q66 0 113-47t47-113q0-66-47-113t-113-47q-66 0-113 47t-47 113q0 66 47 113t113 47Zm0-160Z"/></svg>`;
  }
  if (lowerDesc.includes('mainly clear')) {
    // Cloud SVG
    return `<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M440-760v-160h80v160h-80Zm266 110-56-56 113-114 56 57-113 113Zm54 210v-80h160v80H760Zm3 299L650-254l56-56 114 112-57 57ZM254-650 141-763l57-57 112 114-56 56Zm-14 450h180q25 0 42.5-17.5T480-260q0-25-17-42.5T421-320h-51l-20-48q-14-33-44-52.5T240-440q-50 0-85 35t-35 85q0 50 35 85t85 35Zm0 80q-83 0-141.5-58.5T40-320q0-83 58.5-141.5T240-520q60 0 109.5 32.5T423-400q58 0 97.5 43T560-254q-2 57-42.5 95.5T420-120H240Zm320-134q-5-20-10-39t-10-39q45-19 72.5-59t27.5-89q0-66-47-113t-113-47q-60 0-105 39t-53 99q-20-5-41-9t-41-9q14-88 82.5-144T480-720q100 0 170 70t70 170q0 77-44 138.5T560-254Zm-79-226Z"/></svg>`;
  }
  if (lowerDesc.includes('rain') || lowerDesc.includes('rain showers') || ('drizzle') || lowerDesc.includes('freezing drizzle')) {
    // Rain SVG
    return `<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M558-84q-15 8-30.5 2.5T504-102l-60-120q-8-15-2.5-30.5T462-276q15-8 30.5-2.5T516-258l60 120q8 15 2.5 30.5T558-84Zm240 0q-15 8-30.5 2.5T744-102l-60-120q-8-15-2.5-30.5T702-276q15-8 30.5-2.5T756-258l60 120q8 15 2.5 30.5T798-84Zm-480 0q-15 8-30.5 2.5T264-102l-60-120q-8-15-2.5-30.5T222-276q15-8 30.5-2.5T276-258l60 120q8 15 2.5 30.5T318-84Zm-18-236q-91 0-155.5-64.5T80-540q0-83 55-145t136-73q32-57 87.5-89.5T480-880q90 0 156.5 57.5T717-679q69 6 116 57t47 122q0 75-52.5 127.5T700-320H300Zm0-80h400q42 0 71-29t29-71q0-42-29-71t-71-29h-60v-40q0-66-47-113t-113-47q-48 0-87.5 26T333-704l-10 24h-25q-57 2-97.5 42.5T160-540q0 58 41 99t99 41Zm180-200Z"/></svg>`;
  }
  if (lowerDesc.includes('thunderstorm')) {
    // Thunderstorm SVG
    return `<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="m300-40 36-100h-76l50-140h100l-43 100h83L340-40h-40Zm270-40 28-80h-78l43-120h100l-35 80h82L610-80h-40ZM300-320q-91 0-155.5-64.5T80-540q0-83 55-145t136-73q32-57 87.5-89.5T480-880q90 0 156.5 57.5T717-679q69 6 116 57t47 122q0 75-52.5 127.5T700-320H300Zm0-80h400q42 0 71-29t29-71q0-42-29-71t-71-29h-60v-40q0-66-47-113t-113-47q-48 0-87.5 26T333-704l-10 24h-25q-57 2-97.5 42.5T160-540q0 58 41 99t99 41Zm180-200Z"/></svg>`;
  }
  if (lowerDesc.includes('snow') || lowerDesc.includes('snow fall') || lowerDesc.includes('snow grains') || lowerDesc.includes('snow showers')) {
    // Snow SVG
    return `<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M260-200q-21 0-35.5-14.5T210-250q0-21 14.5-35.5T260-300q21 0 35.5 14.5T310-250q0 21-14.5 35.5T260-200ZM380-80q-21 0-35.5-14.5T330-130q0-21 14.5-35.5T380-180q21 0 35.5 14.5T430-130q0 21-14.5 35.5T380-80Zm120-120q-21 0-35.5-14.5T450-250q0-21 14.5-35.5T500-300q21 0 35.5 14.5T550-250q0 21-14.5 35.5T500-200Zm240 0q-21 0-35.5-14.5T690-250q0-21 14.5-35.5T740-300q21 0 35.5 14.5T790-250q0 21-14.5 35.5T740-200ZM620-80q-21 0-35.5-14.5T570-130q0-21 14.5-35.5T620-180q21 0 35.5 14.5T670-130q0 21-14.5 35.5T620-80ZM300-360q-91 0-155.5-64.5T80-580q0-83 55-145t136-73q32-57 87.5-89.5T480-920q90 0 156.5 57.5T717-719q69 6 116 57t47 122q0 75-52.5 127.5T700-360H300Zm0-80h400q42 0 71-29t29-71q0-42-29-71t-71-29h-60v-40q0-66-47-113t-113-47q-48 0-87.5 26T333-744l-10 24h-25q-57 2-97.5 42.5T160-580q0 58 41 99t99 41Zm180-100Z"/></svg>`;
  }
  if (lowerDesc.includes('fog')) {
    // Fog SVG
    return `<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M720-200q-17 0-28.5-11.5T680-240q0-17 11.5-28.5T720-280q17 0 28.5 11.5T760-240q0 17-11.5 28.5T720-200ZM280-80q-17 0-28.5-11.5T240-120q0-17 11.5-28.5T280-160q17 0 28.5 11.5T320-120q0 17-11.5 28.5T280-80Zm-40-120q-17 0-28.5-11.5T200-240q0-17 11.5-28.5T240-280h360q17 0 28.5 11.5T640-240q0 17-11.5 28.5T600-200H240ZM400-80q-17 0-28.5-11.5T360-120q0-17 11.5-28.5T400-160h280q17 0 28.5 11.5T720-120q0 17-11.5 28.5T680-80H400ZM300-320q-91 0-155.5-64.5T80-540q0-83 55-145t136-73q32-57 87.5-89.5T480-880q90 0 156.5 57.5T717-679q69 6 116 57t47 122q0 75-52.5 127.5T700-320H300Zm0-80h400q42 0 71-29t29-71q0-42-29-71t-71-29h-60v-40q0-66-47-113t-113-47q-48 0-87.5 26T333-704l-10 24h-25q-57 2-97.5 42.5T160-540q0 58 41 99t99 41Zm180-200Z"/></svg>`;
  }
  // Default: Rainbow SVG
  return `<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="var(--color-text)"><path d="M40-280q0-91 34.5-171T169-591q60-60 140-94.5T480-720q91 0 171 34.5T791-591q60 60 94.5 140T920-280h-80q0-149-105.5-254.5T480-640q-149 0-254.5 105.5T120-280H40Zm160 0q0-116 82-198t198-82q116 0 198 82t82 198h-80q0-83-58.5-141.5T480-480q-83 0-141.5 58.5T280-280h-80Z"/></svg>`;
};


const openSmartFeatures = (hour: number) => {
  const event = findEventAtHour(hour)
  if (!event) return

  if (showSmartFeatures.value && smartFeaturesEvent.value?.id === event.id) {
    showSmartFeatures.value = false
    smartFeaturesEvent.value = null
  } else {
    smartFeaturesEvent.value = event
    showSmartFeatures.value = true
  }
}

const closeSmartFeatures = () => {
  showSmartFeatures.value = false
  smartFeaturesEvent.value = null
}

// == Event logic == //
// Map to keep track of pending saves for debouncing
const pendingSaves = new Map<string, ReturnType<typeof setTimeout>>()

// helper function -> Save event function
const saveEvent = (event: CalendarEvent) => {
  // Clear any existing timeout for this event
  const existingTimeout = pendingSaves.get(event.id)
  if (existingTimeout) {
    clearTimeout(existingTimeout)
  }

  // Create new timeout for this event
  const timeout = setTimeout(async () => {
    try {
      await invoke('save_event', { event: JSON.stringify(event) })
      
      try {
        await invoke('trigger_sync')
      } catch (error) {
        console.warn('Failed to trigger sync:', error)
      }
      
      pendingSaves.delete(event.id)
    } catch (error) {
      console.error('Failed to save event:', error)
    }
  }, 1000)

  pendingSaves.set(event.id, timeout)
}

// Helper function to generate a unique key for each hour and date
const getHourKey = (hour: number, date: Date = currentDate.value): string => {
  return `${date.getFullYear()}-${date.getMonth() + 1}-${date.getDate()}-${hour}`;
};

// Function to start editing a specific hour
const startEditing = (hour: number) => {
  if (isInPast(hour) || isNow(hour)) return;

  const key = getHourKey(hour);
  
  // Set the input value if not already set
  if (hourInputs.value[key] === undefined) {
    hourInputs.value[key] = getEventDescription(hour) || '';
  }
  
  activeEditor.value = `editor-${key}`;
  
  // Focus the textarea after it becomes visible
  setTimeout(() => {
    const editor = document.getElementById(`editor-${key}`);
    if (editor) {
      editor.focus();
    }
  }, 0);
};

// Update the event description function
const updateEventDescription = async (event: Event, hour: number) => {
  const target = event.target as HTMLTextAreaElement;
  const value = target.value;
  const key = getHourKey(hour);
  hourInputs.value[key] = value;

  const existingEvent = findEventAtHour(hour);

  if (existingEvent) {
    if (!value.trim()) {
      // Clear any pending saves for this event first
      const existingTimeout = pendingSaves.get(existingEvent.id)
      if (existingTimeout) {
        clearTimeout(existingTimeout)
        pendingSaves.delete(existingEvent.id)
      }
      
      // Immediately update UI to show no event
      events.value = events.value.filter(e => e.id !== existingEvent.id);
      
      // Then delete from backend
      try {
        await invoke('delete_event', { id: existingEvent.id });
        await invoke('trigger_sync');
      } catch (error) {
        console.warn('Failed to delete event:', error);
      }
    } else {
      existingEvent.description = value;
      saveEvent(existingEvent);
    }
  } else if (value.trim()) {
    // Create new event if description is not empty
    const eventDate = new Date(
      currentDate.value.getFullYear(),
      currentDate.value.getMonth(),
      currentDate.value.getDate(),
      hour
    );

    const newEvent = {
      id: crypto.randomUUID(),
      user_id: "",
      description: value,
      time: eventDate.toISOString(),
      alarm: false,
      synced: false,
      synced_google: false,
      deleted: false,
      recurrence: null,
    };
    events.value.push(newEvent);
    saveEvent(newEvent);
  }
};

// Function to handle blur event on hour input //
const handleBlur = (hour: number) => {
  const key = getHourKey(hour);
  const value = hourInputs.value[key]?.trim() || '';
  
  // If the field is empty after trimming, ensure the event is deleted
  if (!value && hasEventAtHour(hour)) {
    deleteEvent(hour);
  }
  
  activeEditor.value = null;
};

// Function to delete an event at a specific hour //
const deleteEvent = async (hour: number) => {
  const existingEvent = findEventAtHour(hour);
  if (existingEvent) {
    // Clear any pending saves for this event first
    const existingTimeout = pendingSaves.get(existingEvent.id);
    if (existingTimeout) {
      clearTimeout(existingTimeout);
      pendingSaves.delete(existingEvent.id);
    }
    
    // Delete the event from backend
    await invoke('delete_event', { id: existingEvent.id });
    
    // Remove from local state
    events.value = events.value.filter(e => e.id !== existingEvent.id);
    
    // Clear the input and editor state for this hour
    const key = getHourKey(hour);
    hourInputs.value[key] = '';
    
    // Reset active editor if it was being edited
    if (activeEditor.value === `editor-${key}`) {
      activeEditor.value = null;
    }
    
    // Trigger sync
    try {
      await invoke('trigger_sync');
    } catch (error) {
      console.warn('Failed to trigger sync after deletion:', error);
    }
  }
};

// helper function -> Schedule native notification helper function
const scheduleNativeNotification = async (event: CalendarEvent) => {
  try {
    await invoke('schedule_event_notification', { 
      event_json: JSON.stringify(event) 
    })
  } catch (error) {
    console.error('Failed to schedule notification:', error)
  }
}

// Set alarm function
const alarm = async (hour: number) => {
  if (isInPast(hour)) return

  const existingEvent = findEventAtHour(hour)

  if (existingEvent) {
    // Toggle alarm for existing event
    existingEvent.alarm = !existingEvent.alarm
    saveEvent(existingEvent)
    
    // Schedule native notifications through Tauri
    if (existingEvent.alarm) {
      scheduleNativeNotification(existingEvent)
    }
    // Trigger sync after alarm change
    try {
      await invoke('trigger_sync')
    } catch (error) {
      console.warn('Failed to trigger sync after alarm change:', error)
    }
  }
}

// == Calendar render + navigaton + interacion logic == //
// functions to controle calendar navigation
const previousMonth = () => {
  currentDate.value = new Date(
    currentDate.value.getFullYear(),
    currentDate.value.getMonth() - 1,
    1
  )
  renderCalendar()
  activeCell.value = null
  const daySchedule = document.querySelector('#day-schedule-container');
  if (daySchedule && daySchedule instanceof HTMLElement) {
    daySchedule.style.display = 'none';
  }
}

const nextMonth = () => {
  currentDate.value = new Date(
    currentDate.value.getFullYear(),
    currentDate.value.getMonth() + 1,
    1
  )
  renderCalendar()
  activeCell.value = null
  const daySchedule = document.querySelector('#day-schedule-container');
  if (daySchedule && daySchedule instanceof HTMLElement) {
    daySchedule.style.display = 'none';
  }
}

// select date function for calendar interaction
const selectDate = (date: CalendarDay) => {
  if (date.date) {
    currentDate.value = new Date(date.date)
    activeCell.value = date.id
    const daySchedule = document.querySelector('#day-schedule-container');
    if (daySchedule && daySchedule instanceof HTMLElement) {
      daySchedule.style.display = 'flex';
    }
  }
}

// render calendar function
const renderCalendar = () => {
  const firstDayOfMonth = new Date(currentDate.value.getFullYear(), currentDate.value.getMonth(), 1)
  const lastDayOfMonth = new Date(currentDate.value.getFullYear(), currentDate.value.getMonth() + 1, 0)
  
  const daysInMonth = lastDayOfMonth.getDate()
  const firstDayIndex = (firstDayOfMonth.getDay() + 6) % 7
  
  calendarDays.value = []
  
  // Fill in the days of the month
  for (let i = 1; i <= daysInMonth; i++) {
    const date = new Date(currentDate.value.getFullYear(), currentDate.value.getMonth(), i)
    calendarDays.value.push({
      id: `${currentDate.value.getFullYear()}-${currentDate.value.getMonth() + 1}-${i}`,
      day: i,
      date: date
    })
  }
  
  // Fill in empty cells before the first day of the month
  for (let i = 0; i < firstDayIndex; i++) {
    calendarDays.value.unshift({ id: `empty-${i}`, day: '', date: null })
  }
  
  // Update month and year display
  currentMonth.value = currentDate.value.toLocaleString('default', { month: 'long' })
  currentYear.value = currentDate.value.getFullYear()
}


// == initialization logic == //
// helper function -> function to load events from database
const loadEvents = async () => {
  try {
    const eventsData = await invoke<string[]>('get_events')
    events.value = eventsData.map(eventStr => JSON.parse(eventStr))
  } catch (error) {
    console.error('Failed to load events:', error)
    events.value = []
  }
}

// Function to fetch weather data for given coordinates
const fetchWeatherData = async (latitude: number, longitude: number) => {
  try {
    const weatherData = await invoke<Record<string, DailyWeather>>('get_weekly_weather', { latitude, longitude });
    weather.value = weatherData;
  } catch (error) {
    weather.value = 'no data';
    console.error('Failed to fetch weather:', error);
  }
}

// helper function -> Refresh events function
const refreshEvents = async () => {
  await invoke('clean_old_events')
  await loadEvents()
}

// Clean up pending saves when component unmounts
onBeforeUnmount(() => {
  for (const timeout of pendingSaves.values()) {
    clearTimeout(timeout)
  }
})

// Initialize calendar on component mount
onMounted(async () => {
  try {
    renderCalendar()
    // Try auto-launch setup but don't fail if it doesn't work
    try {
      await invoke('setup_auto_launch')
    } catch (autoLaunchError) {
      console.warn('Auto-launch setup failed:', autoLaunchError)
    }

    // Listen for location changes from the location component
    listen('location-changed', async (event: any) => {
      const { latitude, longitude } = event.payload;
      await fetchWeatherData(latitude, longitude);
    });

    // Default weather fetch with IP location as fallback
    try {
      const response = await fetch('http://ip-api.com/json/');
      const data = await response.json();
      if (data.status === 'success') {
        await fetchWeatherData(data.lat, data.lon);
      } else {
        weather.value = 'no data';
      }
    } catch (error) {
      weather.value = 'no data';
      console.warn('Could not get default location for weather:', error);
    }
    
    // Clean and load events, but don't block the calendar UI
    await invoke('clean_old_events')
    await loadEvents()
    listen('google_sync_complete', async () => {
    await loadEvents()
    })
    listen('event-saved', async (event) => {
      await loadEvents()
    })
    // Refresh events every minute
    setInterval(refreshEvents, refreshInterval)
    setInterval(async () => {
      await invoke('clean_old_events')
      await loadEvents()
    }, cleanupInterval)
  } catch (error) {
    console.error('Failed to initialize calendar:', error)
  }
})
</script>

<style scoped>
#calendar {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
  width: 100%;
  height: 100%;
  overflow-y: auto;
}

#calendar::-webkit-scrollbar {
  display: none; /* Hide scrollbar for WebKit browsers */
  width: 0 !important;
  height: 0 !important;
  background: transparent !important;
}

/* calendar styles */
#calendar-container {
  background: transparent;
  border-radius: 10px;
  padding: 1rem;
  width: 100%;
  min-width: 490px;
  height: 50%;
  display: flex;
  flex-direction: column;
  align-items: center;
}

#calendar-header {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1rem;
  width: 100%;
}

#calendar-header button {
  background: transparent;
  border: none;
  border-radius: 30%;
  cursor: pointer;
  padding: 0.2rem;
  display: flex;
  align-items: center;
  justify-content: center;
}

#calendar-header button:first-of-type {
  margin-left: auto;
}

#calendar-header button svg {
  opacity: 1;
}

#calendar-grid {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  justify-items: center;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
}

.days-of-week {
  font-weight: bold;
  text-align: center;
}

.calendar-cell {
  border: 1px solid var(--color-border);
  border-radius: 50%;
  padding: 0.5rem;
  width: 50%;
  height: 100%;
  aspect-ratio: 1 / 1;
  min-width: 3rem;
  min-height: 3rem;
  max-width: 4rem;
  max-height: 4rem;
  cursor: pointer;
  display: flex;
  justify-content: center;
  align-items: center;
  position: relative;
}

.calendar-cell:hover {
  box-shadow: 0 0 10px var(--color-theme);
}

.calendar-cell.active {
  background-color: var(--color-theme);
  color: var(--color-dark);
}

.calendar-cell.current-day {
  border: 1px solid var(--color-theme);
}

.indicators {
  position: absolute;
  right: -14px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  justify-content: flex-end;
  align-items: center;
}

.event-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--color-theme);
}

.weather-indicator {
  position: absolute;
  top: -1.5rem;
  right: -0.4rem;
  width: 20px;
  height: 20px;
  border-radius: 50%;
}

/* day schedule styles */
#schedule-container {
  background: var(--color-main);
  border-radius: 10px;
  margin: 1rem;
  width: 95%;
  height: 29rem;
  display: flex;
  flex-direction: column;
}

#schedule-container h3 {
color: var(--color-text);
font-size: 1.2rem;
text-align: center;
margin-bottom: 1rem;
}

#day-schedule-container {
  box-shadow: 0 0 10px var(--color-theme);
  padding: 0.5rem;
  border-radius: 10px;
  width: 100%;
  height: 90%;
  display: flex;
  flex-direction: column;
  align-items: center;
  overflow-y: auto;
  scrollbar-width: none; /* Firefox */
  -ms-overflow-style: none; /* IE and Edge */
}

.hour-wrapper {
  display: flex;
  width: 100%;
  align-items: center;
}

.hour {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: space-between;
  padding-left: 0.3rem;
}

.hour:last-of-type {
  padding-bottom: 0.3rem;
}

.hour span.event-time {
  color: var(--color-theme);
}

.hour hr {
  width: 100%;
  border: 1px solid var(--color-text);
}

.hour hr.event-time {
border-color: var(--color-theme);
}

.hour textarea, .hour .link-display {
width: 100%;
height: 2rem;
font-size: 1rem;
font-family:
  Inter,
  -apple-system,
  BlinkMacSystemFont,
  'Segoe UI',
  Roboto,
  Oxygen,
  Ubuntu,
  Cantarell,
  'Fira Sans',
  'Droid Sans',
  'Helvetica Neue',
  sans-serif;
border: none;
background: var(--color-shadow);
color: var(--color-text);
resize: none;
outline: none;
overflow-y: auto;
scrollbar-width: none; /* Firefox */
-ms-overflow-style: none; /* IE and Edge */
transition: transform 0.3s ease, height 0.3s ease;
margin: 0;
padding: 0.1rem;
min-height: 2rem; /* Ensure minimum height */
cursor: text; /* Show text cursor */
line-height: 1.5;
vertical-align: middle;
box-sizing: border-box;
}

.hour .link-display:empty::before {
opacity: 0.7;
}

.hour textarea::-webkit-scrollbar, .hour .link-display::-webkit-scrollbar {
display: none; /* Chrome, Safari, Opera */
}

.hour textarea.has-event, .hour .link-display.has-event {
color: var(--color-theme);
}

.hour textarea.in-the-past, .hour .link-display.in-the-past {
opacity: 0.5;
cursor: not-allowed;
pointer-events: none;
background-color: var(--color-theme);
}

.hour textarea.expand, .hour .link-display.expand {
display: block;
height: 6rem;
margin: 0;
padding: 0.1rem;
}

.hour-wrapper button {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 0.2rem;
  display: flex;
  align-items: center;
  justify-content: center;
}

.alarm.in-the-past {
  opacity: 0.5;
  cursor: not-allowed;
  pointer-events: none;
}

.alarm:disabled{
  opacity: 0.5;
  cursor: not-allowed;
  pointer-events: none;
}

.delete-btn {
  color: var(--color-text);
}

.expand {
  margin-left: auto;
}
</style>