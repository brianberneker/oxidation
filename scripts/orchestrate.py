import os
import argparse
import requests
import json

def query_local_llm(system_prompt, user_content, api_url, model):
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content}
        ],
        "stream": False
    }
    try:
        response = requests.post(api_url, json=payload)
        response.raise_for_status()
        return response.json().get("message", {}).get("content", "")
    except Exception as e:
        print(f"Error querying LLM: {e}")
        return user_content # Fallback

def main():
    parser = argparse.ArgumentParser(description="Pipe Airwindows C++ through Rust Refactoring Agents.")
    parser.add_argument("cpp_file", help="Path to the source C++ file")
    parser.add_argument("--api-url", default="http://localhost:11434/api/chat", help="Local LLM API URL (Ollama default)")
    parser.add_argument("--model", default="llama3", help="Local LLM Model name")
    parser.add_argument("--output", default="ported_plugin.rs", help="Output file path")
    args = parser.parse_args()

    if not os.path.exists(args.cpp_file):
        print(f"Error: {args.cpp_file} not found.")
        return

    with open(args.cpp_file, "r") as f:
        current_code = f.read()

    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    agents = [
        os.path.join(base_dir, "agents/01_architect_system.md"),
        os.path.join(base_dir, "agents/02_ilp_mathematician_system.md"),
        os.path.join(base_dir, "agents/03_llvm_rustacean_system.md"),
        os.path.join(base_dir, "agents/04_anti_slop_guardrail_system.md")
    ]

    for agent_file in agents:
        print(f"Running agent: {os.path.basename(agent_file)}...")
        with open(agent_file, "r") as f:
            system_prompt = f.read()
        
        current_code = query_local_llm(system_prompt, current_code, args.api_url, args.model)

    with open(args.output, "w") as f:
        f.write(current_code)
    
    print(f"Done! Pipeline complete. Final code saved to {args.output}")

if __name__ == "__main__":
    main()
