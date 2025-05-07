@echo off
setlocal enabledelayedexpansion

:: Script to run three bidirectional agents with directed connections on Windows
:: Agent 1 (port 4200) connects to Agent 2 (port 4201)
:: Agent 2 (port 4201) connects to both Agent 1 (port 4200) and Agent 3 (port 4202)
:: Agent 3 (port 4202) connects to Agent 2 (port 4201)

:: #############################################
:: # Cleanup Functionality (Manual Trigger)    #
:: #############################################
:cleanup
echo Cleaning up and stopping all agents...
:: Kill any bidirectional-agent processes
taskkill /IM bidirectional-agent.exe /F /T >nul 2>&1
echo Cleanup attempt complete. Check task manager if any agents persist.
goto :eof

:: #############################################
:: # Main Script Logic                         #
:: #############################################

:: Set up a way to call cleanup if the script is re-run or at the end
:: Note: True signal trapping like in bash is not directly equivalent.
:: This script will launch agents in new windows. Closing those windows
:: or using Ctrl+C in them will stop those specific agents.
:: This main script will offer a prompt at the end to run a general cleanup.

:: Check if API key is provided
if "%CLAUDE_API_KEY%" == "" (
    if "%GEMINI_API_KEY%" == "" (
        echo Neither CLAUDE_API_KEY nor GEMINI_API_KEY environment variable is set.
        echo Please set one of them before running this script.
        echo Example: set GEMINI_API_KEY=your_api_key_here
        exit /b 1
    )
)

:: Build the agent
echo Building bidirectional agent...
:: Ensure cargo is in PATH
cargo build --release --bin bidirectional-agent || (
    echo Build failed, exiting.
    exit /b 1
)
:: Use release build for potentially better performance/stability, debug also works
set AGENT_EXE_PATH=target\release\bidirectional-agent.exe
if not exist "%AGENT_EXE_PATH%" (
    echo Trying debug build path...
    set AGENT_EXE_PATH=target\debug\bidirectional-agent.exe
    if not exist "%AGENT_EXE_PATH%" (
        echo "Error: bidirectional-agent.exe not found in target\release or target\debug after build."
        exit /b 1
    )
)
echo Using agent executable: %AGENT_EXE_PATH%


:: Directory where the script is located
set PROJECT_DIR=%~dp0
:: Remove trailing backslash if present for cleaner paths
if "%PROJECT_DIR:~-1%"=="\" set PROJECT_DIR=%PROJECT_DIR:~0,-1%

cd /D "%PROJECT_DIR%"

:: Ensure data directory for agent directories exists
mkdir "%PROJECT_DIR%\data" 2>nul

echo Starting bidirectional agents...

:: Create config files for all agents
echo Creating agent1_config.toml...
(
    echo [server]
    echo port = 4200
    echo bind_address = "0.0.0.0"
    echo agent_id = "bidirectional-agent-1"
    echo agent_name = "Agent One"
    echo.
    echo ; [client]
    echo ; target_url = "http://localhost:4201"
    echo.
    echo [llm]
    echo ; API key set via environment variable
    echo system_prompt = "You are a multi-purpose agent that discovers and connects with other agents. Your primary role is to identify when you need assistance from specialized agents and establish connections with them to solve complex tasks collaboratively."
    echo.
    echo [mode]
    echo repl = true
    echo get_agent_card = false
    echo repl_log_file = "shared_agent_interactions.log"
    echo.
    echo [tools]
    echo enabled = ["echo", "llm", "list_agents", "execute_command", "remember_agent"]
    echo ; REMOVED agent_directory_path
) > "%PROJECT_DIR%\agent1_config.toml"

echo Creating agent2_config.toml...
(
    echo [server]
    echo port = 4201
    echo bind_address = "0.0.0.0"
    echo agent_id = "bidirectional-agent-2"
    echo agent_name = "Agent Two"
    echo.
    echo ; [client]
    echo ; target_url = "http://localhost:4200"
    echo.
    echo [llm]
    echo ; API key set via environment variable
    echo system_prompt = "You are a network coordination agent that maintains awareness of available agent services. Your primary function is to connect requestors with the right specialized agents based on the task requirements."
    echo.
    echo [mode]
    echo repl = true
    echo get_agent_card = false
    echo repl_log_file = "shared_agent_interactions.log"
    echo.
    echo [tools]
    echo enabled = ["echo", "llm", "list_agents", "execute_command", "remember_agent"]
    echo ; REMOVED agent_directory_path
) > "%PROJECT_DIR%\agent2_config.toml"

echo Creating agent3_config.toml...
(
    echo [server]
    echo port = 4202
    echo bind_address = "0.0.0.0"
    echo agent_id = "bidirectional-agent-3"
    echo agent_name = "Agent Three -- Memory Specialist"
    echo.
    echo ; [client]
    echo ; target_url = "http://localhost:4201"
    echo.
    echo [llm]
    echo ; API key set via environment variable
    echo system_prompt = "You are a specialized agent with exceptional memory capabilities. Your primary function is to record, store, and recall information about other agents and past interactions, serving as a long-term memory resource for the agent network."
    echo.
    echo [mode]
    echo repl = true
    echo get_agent_card = false
    echo repl_log_file = "shared_agent_interactions.log"
    echo.
    echo [tools]
    echo enabled = ["echo", "summarize", "list_agents", "remember_agent", "execute_command", "llm"]
    echo ; REMOVED agent_directory_path
) > "%PROJECT_DIR%\agent3_config.toml"

:: Start Agent 1
echo Starting Agent 1 in a new window...
set AGENT1_CMD_TITLE="Agent 1 (Port 4200)"
set AGENT1_CMD_COMMAND="cd /D \"%PROJECT_DIR%\" && echo Starting Agent 1 (listening on port 4200, connecting to 4201)... && set RUST_LOG=info && set CLAUDE_API_KEY=%CLAUDE_API_KEY% && set GEMINI_API_KEY=%GEMINI_API_KEY% && set AUTO_LISTEN=true && \"%PROJECT_DIR%\%AGENT_EXE_PATH%\" agent1_config.toml"
start %AGENT1_CMD_TITLE% cmd /K "%AGENT1_CMD_COMMAND%"
echo Agent 1 launching...
timeout /t 3 /nobreak >nul

:: Start Agent 2
echo Starting Agent 2 in a new window...
set AGENT2_CMD_TITLE="Agent 2 (Port 4201)"
set AGENT2_CMD_COMMAND="cd /D \"%PROJECT_DIR%\" && echo Starting Agent 2 (listening on port 4201, connecting to 4200 and 4202)... && set RUST_LOG=info && set CLAUDE_API_KEY=%CLAUDE_API_KEY% && set GEMINI_API_KEY=%GEMINI_API_KEY% && set AUTO_LISTEN=true && \"%PROJECT_DIR%\%AGENT_EXE_PATH%\" agent2_config.toml"
start %AGENT2_CMD_TITLE% cmd /K "%AGENT2_CMD_COMMAND%"
echo Agent 2 launching...
timeout /t 3 /nobreak >nul

:: Start Agent 3
echo Starting Agent 3 in a new window...
set AGENT3_CMD_TITLE="Agent 3 -- Memory Specialist (Port 4202)"
set AGENT3_CMD_COMMAND="cd /D \"%PROJECT_DIR%\" && echo Starting Agent 3 (listening on port 4202, connecting to 4201)... && set RUST_LOG=info && set CLAUDE_API_KEY=%CLAUDE_API_KEY% && set GEMINI_API_KEY=%GEMINI_API_KEY% && set AUTO_LISTEN=true && \"%PROJECT_DIR%\%AGENT_EXE_PATH%\" agent3_config.toml"
start %AGENT3_CMD_TITLE% cmd /K "%AGENT3_CMD_COMMAND%"
echo Agent 3 launching...
timeout /t 1 /nobreak >nul


echo.
echo All agents should now be starting in separate windows.
echo They will listen on their respective ports:
echo - Agent 1: Port 4200 (will connect to Agent 2 on port 4201)
echo - Agent 2: Port 4201 (will connect to both Agent 1 on port 4200 and Agent 3 on port 4202)
echo - Agent 3: Port 4202 (will connect to Agent 2 on port 4201)
echo.
echo All agents are logging at INFO level for detailed output.
echo.
echo Setup for Agent Discovery:
echo 1. First connect the agents to each other (allow a few seconds for agents to fully start):
echo    - In Agent 1 terminal: :connect http://localhost:4201
echo    - In Agent 2 terminal: :connect http://localhost:4200
echo    - In Agent 2 terminal: :connect http://localhost:4202
echo    - In Agent 3 terminal: :connect http://localhost:4201
echo.
echo 2. To discover other agents, use the list_agents tool:
echo    - In any agent terminal: :tool list_agents {\"format\":\"detailed\"}
echo    - This will display all agents this agent knows about.
echo.
echo 3. To check if Agent 2 knows about Agent 3:
echo    - In Agent 1 terminal: :remote :tool list_agents
echo    - This will ask Agent 2 to list all agents it knows about.
echo.
echo 4. To demonstrate agent discovery works:
echo    - Verify Agent 1 doesn't initially know about Agent 3 by running :tool list_agents in Agent 1's window.
echo    - Have Agent 1 ask Agent 2 about other agents using :remote in Agent 1's window.
echo    - After discovering Agent 3, connect directly from Agent 1: :connect http://localhost:4202
echo    - Verify Agent 1 now knows about Agent 3 by running :tool list_agents again in Agent 1's window.
echo.
echo IMPORTANT:
echo To stop all agents, you can close each agent's command window.
echo Alternatively, to attempt a forceful stop of all 'bidirectional-agent.exe' processes:
echo Run the following command in a new Command Prompt, or re-run this script with the 'cleanup' argument:
echo   %~n0 cleanup
echo Or manually:
echo   taskkill /IM bidirectional-agent.exe /F /T
echo.
echo This script will now pause. Press any key to attempt cleanup and exit this script (this will not close agent windows already opened).
pause

:: Check if an argument like "cleanup" was passed to the script directly
if /I "%1"=="cleanup" (
    call :cleanup
    exit /b 0
)

call :cleanup
echo Main script finished.
endlocal
exit /b 0