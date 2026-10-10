#!/usr/bin/env python3
"""
Bulk Model Test - Python script
Test all models from an OpenAI-compatible API and report response times.
"""

import argparse
import asyncio
import json
import os
import time
from typing import Any
from statistics import mean


def get_openai_client(base_url: str, api_key: str):
    """Create an OpenAI-compatible client."""
    from openai import OpenAI
    return OpenAI(
        base_url=base_url.rstrip('/'),
        api_key=api_key,
    )


async def fetch_models(client) -> list[str]:
    """Fetch available models from the API."""
    try:
        models_resp = client.models.list()
        return [m.id for m in models_resp.data if not m.id.startswith('embeddings')]
    except Exception as e:
        print(f"Error fetching models: {e}")
        return []


async def test_model(client, model: str, test_prompt: str = "Hello") -> dict:
    """Test a single model and measure response time."""
    start_time = time.time()
    result = {
        "model": model,
        "response_time_ms": 0,
        "status": "pending",
        "error": None,
        "test_result": None,
    }
    
    try:
        response = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": test_prompt}],
            max_tokens=10,
            temperature=0,
            timeout=30.0,
        )
        elapsed_ms = (time.time() - start_time) * 1000
        result["response_time_ms"] = round(elapsed_ms, 2)
        result["status"] = "success"
        result["test_result"] = response.choices[0].message.content
    except Exception as e:
        elapsed_ms = (time.time() - start_time) * 1000
        result["response_time_ms"] = round(elapsed_ms, 2)
        result["status"] = "error"
        result["error"] = str(e)
    
    return result


async def test_all_models(client, models: list[str], test_prompt: str = "Hello") -> list[dict]:
    """Test all models concurrently."""
    tasks = [test_model(client, model, test_prompt) for model in models]
    results = await asyncio.gather(*tasks)
    return list(results)


def sort_results(results: list[dict]) -> list[dict]:
    """Sort results by response time: successful fast→slow, then errors."""
    success = [r for r in results if r["status"] == "success"]
    error = [r for r in results if r["status"] == "error"]
    
    success.sort(key=lambda x: x["response_time_ms"])
    error.sort(key=lambda x: x["response_time_ms"])
    
    return success + error


def generate_report(results: list[dict] | None = None) -> dict:
    """Generate a JSON report."""
    report = {
        "total_models": 0,
        "successful": 0,
        "errors": 0,
        "avg_response_time_ms": 0,
        "models": [],
    }
    
    if not results:
        return report
    
    report["total_models"] = len(results)
    report["successful"] = sum(1 for r in results if r["status"] == "success")
    report["errors"] = sum(1 for r in results if r["status"] == "error")
    
    successful_times = [r["response_time_ms"] for r in results if r["status"] == "success"]
    if successful_times:
        report["avg_response_time_ms"] = round(mean(successful_times), 2)
    
    for r in results:
        report["models"].append({
            "name": r["model"],
            "response_time_ms": r["response_time_ms"],
            "status": r["status"],
            "error": r["error"],
        })
    
    return report


def print_report(report: dict):
    """Print a formatted report to console."""
    print("\n" + "=" * 60)
    print("BULK MODEL TEST REPORT")
    print("=" * 60)
    print(f"Total Models: {report['total_models']}")
    print(f"Successful: {report['successful']}")
    print(f"Errors: {report['errors']}")
    if report['avg_response_time_ms'] > 0:
        print(f"Avg Response Time: {report['avg_response_time_ms']} ms")
    print("-" * 60)
    print("\nResults (Fast → Slow, Errors Last):\n")
    
    for model in report["models"]:
        if model["status"] == "success":
            print(f"  ✓ {model['name']:<40} {model['response_time_ms']:>10.2f} ms")
        else:
            print(f"  ✗ {model['name']:<40} {model['response_time_ms']:>10.2f} ms (ERROR)")
            print(f"      └─ {model['error']}")
    
    print("\n" + "=" * 60)


def generate_chart_data(results: list[dict]) -> dict:
    """Generate data for a response time chart."""
    chart_data = {
        "labels": [],
        "values": [],
        "colors": [],
    }
    
    for r in results:
        chart_data["labels"].append(r["model"])
        chart_data["values"].append(r["response_time_ms"])
        chart_data["colors"].append("#4CAF50" if r["status"] == "success" else "#f44336")
    
    return chart_data


def main():
    parser = argparse.ArgumentParser(description="Bulk Model Test - Test all models from an OpenAI-compatible API")
    parser.add_argument("--base-url", required=True, help="Base URL of the OpenAI-compatible API")
    parser.add_argument("--api-key", required=True, help="API Key for authentication")
    parser.add_argument("--model", help="Specific model name to test (optional, auto-detect if not provided)")
    parser.add_argument("--test-prompt", default="Hello", help="Test prompt for LLM request")
    parser.add_argument("--auto-detect", action="store_true", help="Auto-detect and list all available models")
    parser.add_argument("--output", "-o", help="Output JSON file path for report")
    parser.add_argument("--chart-data", help="Output JSON file path for chart data")
    
    args = parser.parse_args()
    
    print(f"\nConnecting to: {args.base_url}")
    print(f"Testing with prompt: '{args.test_prompt}'")
    print(f"Auto-detect models: {args.auto_detect or not args.model}")
    
    client = get_openai_client(args.base_url, args.api_key)
    
    models_to_test = []
    
    if args.model:
        models_to_test = [args.model]
        print(f"\nTesting specific model: {args.model}")
    else:
        print("\nFetching available models...")
        models = asyncio.run(fetch_models(client))
        if not models:
            print("Error: Could not fetch models")
            return 1
        models_to_test = models
        print(f"Found {len(models)} models to test")
        for m in models[:5]:
            print(f"  - {m}")
        if len(models) > 5:
            print(f"  ... and {len(models) - 5} more")
    
    print(f"\nTesting {len(models_to_test)} model(s)...\n")
    
    results = asyncio.run(test_all_models(client, models_to_test, args.test_prompt))
    sorted_results = sort_results(results)
    
    report = generate_report(sorted_results)
    print_report(report)
    
    if args.output:
        with open(args.output, 'w') as f:
            json.dump(report, f, indent=2)
        print(f"\nReport saved to: {args.output}")
    
    if args.chart_data:
        chart_data = generate_chart_data(sorted_results)
        with open(args.chart_data, 'w') as f:
            json.dump(chart_data, f, indent=2)
        print(f"Chart data saved to: {args.chart_data}")
    
    return 0


if __name__ == "__main__":
    exit(main())