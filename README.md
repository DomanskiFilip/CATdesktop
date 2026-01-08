# CAT - Calendar Assistant (v1.0.0-beta)

**CAT (Calendar AssistanT)** is an intelligent desktop calendar application featuring a conversational AI assistant powered by Claude 3 Haiku. This cross-platform application helps users manage their schedules through natural language interactions while maintaining strong privacy through end-to-end encryption. Connect your Google and Outlook calendars. Draft event related emails with AI email agent, plan route for your event with localisation based AI route agent and inbuilt map with car and public transport modes and "leave before x" smart notifications. Stay up to date with your events thanks to native notifications on all platforms and cross platform bidirectional sync.

![Tauri](https://img.shields.io/badge/Tauri-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![Vue.js](https://img.shields.io/badge/Vue.js-35495E?style=for-the-badge&logo=vue.js&logoColor=4FC08D)
![AWS](https://img.shields.io/badge/AWS-232F3E?style=for-the-badge&logo=amazon-aws&logoColor=white)
![AWS Lambda](https://img.shields.io/badge/AWS%20Lambda-%23FF9900.svg?style=for-the-badge&logo=aws-lambda&logoColor=white)
![Amazon DynamoDB](https://img.shields.io/badge/Amazon%20DynamoDB-4053D6?style=for-the-badge&logo=Amazon%20DynamoDB&logoColor=white)
![AWS Bedrock](https://img.shields.io/badge/AWS%20Bedrock-%23232F3E.svg?style=for-the-badge&logo=amazon-aws&logoColor=white)

## Key Features

### Intelligent AI Assistant
- **Natural Language Processing:** Create, update, delete, and move events using conversational commands
- **Contextual Understanding:** AI remembers conversation history and learns from user preferences
- **Smart Scheduling:** Automatically detects time conflicts and suggests optimal scheduling
- **Weather Integration:** Provides weather-aware event suggestions and recommendations
- **Personalized Responses:** Adapts communication style based on user interaction patterns

### Privacy & Security
- **End-to-End Encryption:** All calendar data encrypted using ChaCha20-Poly1305 encryption
- **Local Data Storage:** SQLite database with encrypted event descriptions
- **JWT Authentication:** Secure token-based authentication with device verification
- **Privacy-First Design:** No personal data stored unencrypted on servers

### Multi-Platform Sync
- **Google Calendar Integration:** Bidirectional sync with Google Calendar
- **Outlook Integration:** Bidirectional sync with Outlook Calendar
- **Cross-Device Support:** Desktop (Windows, macOS, Linux), Android, and iOS
- **Real-time Sync:** Automatic synchronization across all connected platforms

### Smart Features
- **Location Services:** Integrated Google Maps for route planning and location suggestions
- **Email Generation:** Automatically compose and send event-related emails to participants
- **Voice Input:** Speech-to-text functionality for hands-free event creation
- **Smart Notifications:** Intelligent notification scheduling with customizable lead times

### User Experience
- **Modern UI:** Clean, responsive interface with multiple theme options
- **Mobile-First:** Optimized for both desktop and mobile platforms
- **Conflict Resolution:** Interactive dialogs for handling scheduling conflicts
- **Quick Suggestions:** Pre-built command suggestions for common tasks
- **Real-time Chat:** Instant AI responses with typing indicators

## AI Agent Workflows

#### 1. Calendar Assistant Agent
Handles natural language calendar management (create, update, delete, query events, detect and resolve conflicts, etc.)

#### 2. Event Enrichment Agent
Enhances events with additional context and information (location, weather, missing details, recommendations).

#### 3. Email Generation Agent
Automates event-related communication, including invitations, reminders, and cancellations.

#### 4. Memory & Learning System
Learns user patterns and preferences to personalize scheduling and suggestions.

#### 5. Self Improvement
The system proactively analyzes historical user interactions to predict and avoid potentially rejected outputs, dynamically refining AI-generated responses to better align with individual user preferences prior to presentation.

## Rate Limiting & Fair Usage
- **Daily Limits:** 25 AI requests per user per day
- **Daily Limits:** 5 voice tanscription requests per user per day
- **Intelligent Caching:** Reduces API calls through smart caching
- **Graceful Degradation:** Basic features available if limits are reached

## Technical Architecture
- **Frontend:** Vue.3 + TypeScript
- **Backend:** Rust (Tauri framework)
- **AI Processing:** AWS Lambda + AWS DynamoDB + Claude 3 Haiku
- **Database:** SQLite with encrypted storage
- **Sync Services:** Google/Outlook OAuth integration
- **Cross-Platform:** Tauri for native desktop and mobile support
## Cat Architecture Diagram
<img width="1505" height="2014" alt="app architecture diagram" src="https://github.com/user-attachments/assets/d8fb4503-24e2-4e4c-bf69-c7bc8a6be869" />


## Supported Platforms
- **Desktop:** Windows 10/11, macOS 10.15+, Linux
- **Mobile:** Android 8.0+ (minsdk 26), iOS 18.6+

## Beta Version Notes
- **See Releases tab for more information about specific versions**
- **App size:** 20.4 MB.

---

## License
Copyright (c) 2025 Filip Domanski

CATdesktop is provided for personal, non-commercial use only. You may view, download, and run the application or source code for your own personal purposes.

You may not:
- Redistribute, host, or publish the code or application in any form, whether modified or unmodified.
- Monetize, sell, or offer the code or application as a service.
- Create forks or derivative works for distribution or public hosting.
- Use the code or application in any commercial context.

Filip Domanski is the exclusive host and distributor of CATdesktop. For commercial or redistribution inquiries, contact the copyright holder.

All rights reserved.

## Contact
For beta access or questions, contact the creator (Filip Domanski).
