<template>
  <!-- time debug help: <span style="position: fixed;">{{ currentDate }}</span> -->
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
        <div 
          v-for="date in calendarDays" 
          :key="date.id"
          class="calendar-cell"
          :class="{
            'current-day': isToday(date),
            'active': activeCell === date.id 
          }"
          @click="selectDate(date)"
        >
          {{ date.day }}
          <div class="event-indicators" v-if="getEventsForDate(date).length > 0">
            <span class="event-dot"></span>
          </div>
        </div>
      </section>
    </section>
    <section id="day-schedule-container">
      <h3>{{ currentDate.toLocaleDateString('default', { month: 'long', day: 'numeric', year: 'numeric' }) }} Schedule</h3>
      <span v-for="hour in Array.from({ length: 24 }, (_, i) => (i + 1) % 24)" 
        :key="hour" 
        class="hour">
        <span :class="{ 'event-time': hasEventAtHour(hour) }">{{ hour < 10 ? '0' + hour : hour }}:00</span>
        <textarea 
          :value="getEventDescription(hour)"
          @input="updateEventDescription($event, hour)"
          :class="{ 'has-event': hasEventAtHour(hour) }"
          name="description"
        ></textarea>
        <hr :class="{ 'event-time': hasEventAtHour(hour) }">
      </span>
    </section>
   </section>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface CalendarEvent {
  id: string
  title: string
  description: string
  Time: Date
}

// State management
const currentDate = ref(new Date())
const events = ref<CalendarEvent[]>([])
const activeCell = ref<string | null>(null)

// Calendar logic functions
const daysOfWeek = ['M', 'T', 'W', 'T', 'F', 'S', 'S']
const currentMonth = ref(currentDate.value.toLocaleString('default', { month: 'long' }))
const currentYear = ref(currentDate.value.getFullYear())

// interface for calendar days
interface CalendarDay {
  id: string
  day: number | string
  date: Date | null
}
const calendarDays = ref<CalendarDay[]>([])

// current day
const isToday = (date: CalendarDay) => {
  const today = new Date()
  return date.date && date.date.getDate() === today.getDate() &&
         date.date.getMonth() === today.getMonth() &&
         date.date.getFullYear() === today.getFullYear()
}

// previous and next month functions
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

// select date function
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

// Get events for specific date
const getEventsForDate = (date: CalendarDay) => {
  if (!date.date) return []
  return events.value.filter(event => {
    const eventDate = new Date(event.Time)
    return eventDate.getDate() === date.date?.getDate() &&
           eventDate.getMonth() === date.date?.getMonth() &&
           eventDate.getFullYear() === date.date?.getFullYear()
  })
}

// Check if there is an event at a specific hour
const hasEventAtHour = (hour: number) => {
  return events.value.some(event => {
    const eventHour = new Date(event.Time).getHours()
    const eventDate = new Date(event.Time)
    return eventHour === hour && 
           eventDate.getDate() === currentDate.value.getDate() &&
           eventDate.getMonth() === currentDate.value.getMonth() &&
           eventDate.getFullYear() === currentDate.value.getFullYear()
  })
}

// Get event description for a specific hour
const getEventDescription = (hour: number) => {
  const event = events.value.find(event => {
    const eventHour = new Date(event.Time).getHours()
    const eventDate = new Date(event.Time)
    return eventHour === hour && 
           eventDate.getDate() === currentDate.value.getDate() &&
           eventDate.getMonth() === currentDate.value.getMonth() &&
           eventDate.getFullYear() === currentDate.value.getFullYear()
  })
  return event ? event.description : ''
}

const updateEventDescription = (event: Event, hour: number) => {
  const value = (event.target as HTMLTextAreaElement).value
  const existingEvent = events.value.find(event => {
    const eventHour = new Date(event.Time).getHours()
    const eventDate = new Date(event.Time)
    return eventHour === hour && 
           eventDate.getDate() === currentDate.value.getDate() &&
           eventDate.getMonth() === currentDate.value.getMonth() &&
           eventDate.getFullYear() === currentDate.value.getFullYear()
  })

  if (existingEvent) {
    existingEvent.description = value
  } else if (value) {
    // Create new event if text is entered in empty textarea
    events.value.push({
      id: crypto.randomUUID(),
      title: `Event at ${hour}:00`,
      description: value,
      Time: new Date(
        currentDate.value.getFullYear(),
        currentDate.value.getMonth(),
        currentDate.value.getDate(),
        hour
      )
    })
  }
}

onMounted(() => {
  renderCalendar()
  // Add sample event
  events.value.push({
    id: '1',
    title: 'Test Event 1',
    description: 'Morning Meeting',
    Time: new Date(2025, 5, 10, 10, 0, 0), // June 10, 2025 at 10:00
  })

  events.value.push({
    id: '2',
    title: 'Test Event 2',
    description: 'Lunch with Team',
    Time: new Date(2025, 5, 15, 13, 0, 0), // June 15, 2025 at 13:00
  })

  events.value.push({
    id: '3',
    title: 'Test Event 3',
    description: 'Project Deadline',
    Time: new Date(2025, 5, 25, 16, 0, 0), // June 25, 2025 at 16:00
  })
})
</script>

<style scoped>
#calendar {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
  width: 100%;
  height: 100%;
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
#day-schedule-container {
  background: var(--color-main);
  border: 1px solid var(--color-theme);
  border-radius: 10px;
  width: 16rem;
  height: 25rem;
  margin-top: 1rem;;
  display: flex;
  flex-direction: column;
  align-items: center;
  overflow-y: auto;
  position: relative;
  scrollbar-width: none; /* Firefox */
  -ms-overflow-style: none; /* IE and Edge */
}

#day-schedule-container h3 {
  position: fixed;
  top: -0.1rem;
  color: var(--color-text);
  font-size: 1.2rem;
  text-align: center;
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
  }

    .hour textarea::-webkit-scrollbar {
      display: none; /* Chrome, Safari, Opera */
    }
    
    .hour textarea.has-event {
    color: var(--color-theme);
    }
</style>