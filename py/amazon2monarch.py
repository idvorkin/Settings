#!uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "typer",
#     "rich",
#     "icecream",
# ]
# ///

import csv
import typer
from datetime import datetime
import os
from rich import print
from icecream import ic

key_website = "Website"
key_order_date = "Order Date"
key_total_cost = "Total Owed"
key_name = "Product Name"
key_payment_type = "Payment Instrument Type"

whole_foods_value = "panda01"
amazon_com_value = "Amazon.com"

app = typer.Typer(no_args_is_help=True)


def read_csv_with_bom_handling(filepath) -> csv.DictReader:
    return csv.DictReader(open(filepath, mode="r", encoding="utf-8-sig"))


def input_categories():
    return "Groceries;Electronics;Pets;Entertainment & Recreation;Clothing;Furniture & Housewares;Shopping".split(
        ";"
    )


@app.command(help="Prompt and display available categories.")
def prompt_categories():
    ic(input_categories())


@app.command(
    help="Extract all product titles into an output file for category mapping."
)
def extract_titles(
    input_csv: typer.FileText = typer.Option(
        "Retail.OrderHistory.1.csv",
        help="Path of the input CSV. This is usually the Retail.OrderHistory.1.csv file from your Amazon Data Request.",
    ),
    output_file: typer.FileTextWrite = typer.Option(
        "titles.txt",
        help="Path of the output file where product titles will be written.",
    ),
):
    print("\nExtracting product titles...")
    csv_reader = read_csv_with_bom_handling(input_csv.name)
    titles = {}

    for row in csv_reader:
        price = float(row[key_total_cost].replace(",", ""))
        titles[row[key_name]] = price

    for title in sorted(titles, key=titles.get):
        output_file.write(f"{title}\n")

    print(f"Extracted {len(titles)} unique product titles to {output_file.name}.")

    input_csv.close()
    output_file.close()


def convert(
    inputfile,
    outputfile,
    since_date,
    output_category,
    output_account,
    output_tags,
    output_notes_prefix,
):
    print("\n")
    print("Input CSV file is: ", inputfile.name)
    print("Output CSV will be written at: ", outputfile.name)

    if since_date is None:
        since_date = datetime(2021, 1, 1)
    print("Transactions after this date will be processed: ", since_date.date())

    print("\n")
    input("Press Enter to continue, or Control+C to abort...")

    amazon_transactions = 0
    whole_foods_transactions = 0
    zero_cost_transactions = 0

    csv_reader = read_csv_with_bom_handling(inputfile.name)
    ic(csv_reader.fieldnames)

    output_field_names = [
        "Date",
        "Merchant",
        "Category",
        "Account",
        "Original Statement",
        "Notes",
        "Amount",
    ]
    csv_writer = csv.DictWriter(outputfile, output_field_names)
    csv_writer.writeheader()

    for row in csv_reader:
        if row[key_website] == whole_foods_value:
            whole_foods_transactions += 1
        elif row[key_website] == amazon_com_value:
            transaction_date = strptime(row[key_order_date])
            if row[key_total_cost] == "0":
                zero_cost_transactions += 1
            elif since_date.date() <= transaction_date.date():
                amazon_transactions += 1
                csv_writer.writerow(
                    {
                        "Date": str(strptime(row[key_order_date]).date()),
                        "Merchant": row[key_name][0:31] + "-AMZN",
                        "Category": output_category,
                        "Account": output_account,
                        "Original Statement": row[key_name],
                        "Notes": output_notes_prefix + row[key_payment_type],
                        "Amount": "-" + row[key_total_cost],
                    }
                )

    print("\n")
    print(
        f"Processed {amazon_transactions} Amazon.com transactions since {str(since_date.date())}."
    )
    print(f"Ignored {whole_foods_transactions} Whole Foods transactions.")
    print(f"Ignored {zero_cost_transactions} transactions with $0 order amounts.")


def strptime(str):
    if "." not in str:
        return datetime.strptime(str, "%Y-%m-%dT%H:%M:%S%z")
    else:
        return datetime.strptime(str, "%Y-%m-%dT%H:%M:%S.%f%z")


def arg_date(str):
    try:
        return datetime.strptime(str, "%Y-%m-%d")
    except ValueError:
        msg = "not a valid date: {0!r}".format(str)
        raise ValueError(msg)


@app.command(help="Convert Amazon order history CSV to Monarch-compatible CSV.")
def main(
    input_csv: typer.FileText = typer.Option(
        "Retail.OrderHistory.1.csv",
        help="Path of the input CSV. This is usually the Retail.OrderHistory.1.csv file from your Amazon Data Request.",
    ),
    output_csv: typer.FileTextWrite = typer.Option(
        "monarch.csv",
        help="Path of the output CSV. This is the file you will upload to Monarch.",
    ),
    since_date: datetime = typer.Option(
        None,
        help="Process transactions past since this date. If not provided, will process all transactions.",
    ),
    output_category: str = typer.Option(
        "Uncategorized",
        help="Set the category of all transactions in the output file. Defaults to 'Uncategorized'.",
    ),
    output_account: str = typer.Option(
        "Amazon Gift Card Balance",
        help="Name of the account on Monarch for all the processed transactions. Defaults to 'Amazon Gift Card Balance'.",
    ),
    output_tags: str = typer.Option(
        "input-converter",
        help="Comma separated list of Tags on Monarch to be added to all transactions. Defaults to 'input-converter'.",
    ),
    output_notes_prefix: str = typer.Option(
        "Payment Method: ",
        help="Prefix text to be added to transaction notes in the output files. Payment method is added by default, and the prefix 'Payment Method: ' is added.",
    ),
):
    try:
        convert(
            input_csv,
            output_csv,
            since_date,
            output_category,
            output_account,
            output_tags,
            output_notes_prefix,
        )
    except Exception as e:
        print("\nUh Oh. Script failed because of the following reason: ")
        print(e)
        ic(e)
        input_csv.close()
        output_csv.close()
        os.remove(output_csv.name)
    finally:
        input_csv.close()
        output_csv.close()
        epilog = "Thank you for using this script. Hope it helped you. Cheers! 🍻"
        print("\n")
        print(epilog)


if __name__ == "__main__":
    app()
