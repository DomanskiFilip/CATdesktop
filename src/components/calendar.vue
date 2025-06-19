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
          <div class="event-indicators" v-if="getEventsForDate(date).length > 0">
            <span class="event-dot"></span>
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
             <button class="expand" @click="toggleExpand(hour)" title="expand/collapse">
              <svg v-if="!expand[hour]" xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M480-344 240-584l56-56 184 184 184-184 56 56-240 240Z"/></svg>
              <svg v-else xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" fill="var(--color-text)"><path d="M480-528 296-344l-56-56 240-240 240 240-56 56-184-184Z"/></svg>
             </button>
          </span>
          <textarea :value="getEventDescription(hour)" @input="updateEventDescription($event, hour)" :class="{ 'in-the-past': isInPast(hour), 'has-event': hasEventAtHour(hour), expand: expand[hour] }" :readonly="isInPast(hour) || isNow(hour)" name="description" title="describe the event"></textarea>
          <hr :class="{ 'event-time': hasEventAtHour(hour) }"> 
        </span>
      </section>
    </section>
   </section>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'

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

// function to update and save event description
const updateEventDescription = async (event: Event, hour: number) => {
  const value = (event.target as HTMLTextAreaElement).value
  const existingEvent = findEventAtHour(hour)

  if (existingEvent) {
    if (!value.trim()) {
      // Delete event if description is empty
      invoke('delete_event', { id: existingEvent.id })
      events.value = events.value.filter(e => e.id !== existingEvent.id)
      // Trigger sync after update
       try {
        await invoke('trigger_sync')
      } catch (error) {
        console.warn('Failed to trigger sync after deletion:', error)
      }
    } else {
      existingEvent.description = value
      saveEvent(existingEvent)
      // Trigger sync after update
      try {
        await invoke('trigger_sync')
      } catch (error) {
        console.warn('Failed to trigger sync after update:', error)
      }
    }
  } else if (value.trim()) {
    // Create new event if description is not empty
    const eventDate = new Date(
      currentDate.value.getFullYear(),
      currentDate.value.getMonth(), 
      currentDate.value.getDate(),
      hour
    )
    
    const newEvent = {
      id: crypto.randomUUID(),
      user_id: "",
      description: value,
      time: eventDate.toISOString(),
      alarm: false,
      synced: false,
      deleted: false
    }
    events.value.push(newEvent)
    saveEvent(newEvent)
    // Trigger sync after creation
    try {
      await invoke('trigger_sync')
    } catch (error) {
      console.warn('Failed to trigger sync after creation:', error)
    }
  }
}

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
  const firstDayIndex = firstDayOfMonth.getDay()
  
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
    
    // Clean and load events, but don't block the calendar UI
    await invoke('clean_old_events')
    await loadEvents()

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
  width: 64rem;
  height: 30rem;
  padding-top: 1rem;
}

/* calendar styles */
#calendar-container {
  background: transparent;
  border-radius: 10px;
  padding: 1rem;
  width: 32rem;
}

#calendar-header {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1rem;
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
  grid-template-columns: repeat(7, 4rem);
  justify-content: center;
  align-items: center;
  gap: 0.5rem;
}

.days-of-week {
  font-weight: bold;
  text-align: center;
}

.calendar-cell {
  border: 1px solid var(--color-border);
  border-radius: 50%;
  padding: 0.5rem;
  width: 3rem;
  height: 3rem;
  cursor: pointer;
  display: flex;
  justify-content: center;
  align-items: center;
  position: relative;
  margin-left: 8px;
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

.event-indicators {
  position: absolute;
  right: -12px;
  display: flex;
  gap: 2px;
  justify-content: center;
}

.event-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--color-theme);
}

/* day schedule styles */
#schedule-container {
  background: transparent;
  border-radius: 10px;
  padding: 1rem;
  width: 32rem;
  height: 30rem;
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
  border: 1px solid var(--color-theme);
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

  .hour textarea {
    width: 100%;
    height: 2rem;
    border: none;
    background: transparent;
    color: var(--color-text);
    resize: none;
    outline: none;
    overflow-y: auto;
    scrollbar-width: none; /* Firefox */
    -ms-overflow-style: none; /* IE and Edge */
    transition: transform 0.3s ease, height 0.3s ease;
  }

    .hour textarea::-webkit-scrollbar {
      display: none; /* Chrome, Safari, Opera */
    }
    
    .hour textarea.has-event {
      color: var(--color-theme);
    }

    .hour textarea.in-the-past {
      opacity: 0.5;
      cursor: not-allowed;
      pointer-events: none;
      background-color: var(--color-theme);
    }

    .hour textarea.expand {
      height: 6rem;
      max-height: 200px;
      overflow-y: auto;
    }

.alarm {
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

.expand {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 0.2rem;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-left: auto;
}
</style>