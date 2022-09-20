#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(half_open_range_patterns)]
#![feature(precise_pointer_size_matching)]
#![feature(is_sorted)]
#![warn(
    clippy::suspicious,
    clippy::pedantic,
    clippy::style,
    clippy::complexity,
    clippy::nursery,
    clippy::cargo
)]

use crate::Category::{Cheese, Drinks, OilsAndFats};
use bauxite::BoxBuilder;
use clap::Parser;
use dialoguer::{Confirm, Input, Select};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::borrow::Cow;
use std::fmt::Display;
use std::io;
use std::str::FromStr;
use strum::{EnumCount, EnumIter, EnumVariantNames, IntoEnumIterator, VariantNames};

#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumVariantNames, EnumIter, EnumCount)]
enum Category {
    Drinks,
    Cheese,
    #[strum(to_string = "Oils And Fats")]
    OilsAndFats,
    Other,
}

impl Category {
    const fn score_to_letter(self, score: isize, is_water: bool) -> char {
        match self {
            Drinks => match score {
                _ if is_water => 'A',
                ..=1 => 'B',
                2..=5 => 'C',
                6..=9 => 'D',
                10.. => 'E',
            },
            _ => match score {
                ..=-1 => 'A',
                0..=2 => 'B',
                3..=10 => 'C',
                11..=18 => 'D',
                19.. => 'E',
            },
        }
    }

    fn all_cutoffs(&self) -> [&[f32]; 7] {
        let energy = if *self == Drinks {
            &[
                0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0, 210.0, 240.0, 270.0,
            ]
        } else {
            &ENERGY_CUTOFFS
        };
        let fats = if *self == OilsAndFats {
            &[10.0, 16.0, 22.0, 28.0, 34.0, 40.0, 46.0, 52.0, 58.0, 64.0] // percentages of saturated fats / all fats
        } else {
            &SATURATED_FATS_CUTOFF
        };
        let sugar = if *self == Drinks {
            &[0.0, 1.5, 3.0, 4.5, 6.0, 7.5, 9.0, 10.5, 12.0, 13.5]
        } else {
            &SUGAR_CUTOFFS
        };
        let fruits = if *self == Drinks {
            &[0.0, 40.0, 40.0, 60.0, 60.0, 80.0, 80.0, 80.0, 80.0, 80.0]
        } else {
            &FRUITS_CUTOFFS
        };
        [
            energy,
            fats,
            sugar,
            &PROTEIN_CUTOFFS,
            &SODIUM_CUTOFF,
            &FIBERS_CUTOFFS,
            fruits,
        ]
    }
}

#[derive(Debug)]
struct Nutrition {
    energy: f32,
    fat: f32,
    saturated_fats: f32,
    sugar: f32,
    proteins: f32,
    salt: f32,
    fibers: f32,
}

#[derive(Debug, Parser)]
struct NutritionArgs {
    energy: Option<f32>,
    fat: Option<f32>,
    saturated_fats: Option<f32>,
    sugar: Option<f32>,
    proteins: Option<f32>,
    salt: Option<f32>,
    fibers: Option<f32>,
}

#[derive(Parser)]
struct X {
    x: Option<f32>,
}

impl Nutrition {
    fn saturated_fat_value(&self, cat: Category) -> f32 {
        if cat == OilsAndFats {
            self.saturated_fats / self.fat * 100.0
        } else {
            self.saturated_fats
        }
    }

    fn sodium(&self) -> f32 {
        self.salt / 2.5
    }
}

// negative
static ENERGY_CUTOFFS: [f32; 10] = [
    335.0, 670.0, 1005.0, 1340.0, 1675.0, 2010.0, 2345.0, 2680.0, 3015.0, 3350.0,
];
static SUGAR_CUTOFFS: [f32; 10] = [4.5, 9.0, 13.5, 18.0, 22.5, 27.0, 31.0, 36.0, 40.0, 45.0];
static SATURATED_FATS_CUTOFF: [f32; 10] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
static SODIUM_CUTOFF: [f32; 10] = [
    90.0, 180.0, 270.0, 360.0, 450.0, 540.0, 630.0, 720.0, 810.0, 900.0,
];

// positive
static FRUITS_CUTOFFS: [f32; 10] = [
    40.0,
    60.0,
    80.0,
    80.0,
    80.0,
    f32::INFINITY,
    f32::INFINITY,
    f32::INFINITY,
    f32::INFINITY,
    f32::INFINITY,
];
static FIBERS_CUTOFFS: [f32; 5] = [0.8, 1.9, 2.8, 3.7, 4.7];
static PROTEIN_CUTOFFS: [f32; 5] = [1.6, 3.2, 4.8, 6.4, 8.0];

fn main() -> io::Result<()> {
    let args: NutritionArgs = NutritionArgs::parse();
    let nutrition = Nutrition {
        energy: args.energy.unwrap_or_else(|| ask("Energy (kJ)")),
        fat: args.fat.unwrap_or_else(|| ask("Fats")),
        saturated_fats: args.saturated_fats.unwrap_or_else(|| ask("Saturated fats")),
        sugar: args.sugar.unwrap_or_else(|| ask("Sugar")),
        proteins: args.proteins.unwrap_or_else(|| ask("Protein")),
        salt: args.salt.unwrap_or_else(|| ask("Salt")),
        fibers: args.fibers.unwrap_or_else(|| ask("Fibers")),
    };
    let category: Category = ask_enum("Category")?;
    let fruits: f32 = ask("Percentage of fruits and vegetables");
    let is_water: bool = if category == Drinks {
        Confirm::new().with_prompt("Is it water").interact()?
    } else {
        false
    };

    let score = calculate_nutriscore(category, &nutrition, fruits);
    let letter = category.score_to_letter(score, is_water);

    println!("\nTotal Score:");
    println!("{}", BoxBuilder::new(format!("{letter}")));

    Ok(())
}

fn ask<T>(prompt: &str) -> T
where
    T: Clone + FromStr + Display,
    <T as FromStr>::Err: Display,
{
    Input::new().with_prompt(prompt).interact().unwrap()
}

fn ask_enum<T: VariantNames + IntoEnumIterator + EnumCount>(prompt: &str) -> io::Result<T>
where
    [(); T::COUNT - 1]:,
{
    let idx = Select::new()
        .items(T::VARIANTS)
        .with_prompt(prompt)
        .default(T::COUNT - 1)
        .interact()?;
    Ok(T::iter().nth(idx).unwrap())
}

fn points<T>(arr: &[T], value: &T) -> usize
where
    T: PartialOrd,
{
    assert!(arr.is_sorted());
    let idx: usize = arr.iter().rposition(|c| value > c).map_or(0, |n| n + 1);
    assert!(idx <= arr.len());
    idx
}

fn calculate_nutriscore(cat: Category, nutrition: &Nutrition, fruits_value: f32) -> isize {
    let [energy, fats, sugar, protein, sodium, fibers, fruits] = cat.all_cutoffs();
    let fat_value = nutrition.saturated_fat_value(cat);
    let negative = draw_negative("Energy", energy, &nutrition.energy)
        + draw_negative("Sugar", sugar, &nutrition.sugar)
        + draw_negative("Fats", fats, &fat_value)
        + draw_negative("Sodium", sodium, &nutrition.sodium());
    let negative = isize::try_from(negative).unwrap();
    let fruits_points = draw_positive("Fruits & Vegs", fruits, &fruits_value);
    let positive = || {
        isize::try_from(
            fruits_points
                + draw_positive("Fibers", fibers, &nutrition.fibers)
                + draw_positive("Protein", protein, &nutrition.proteins),
        )
        .unwrap()
    };
    if cat == Cheese {
        negative - positive()
    } else if negative >= 11 && fruits_points < 5 {
        println!("\nThe negative score {negative} is more than 10 and the fruit score {fruits_points} is less than 5.");
        println!("Fibers and Proteins will not be counted!");
        negative - isize::try_from(fruits_points).unwrap()
    } else {
        negative - positive()
    }
}

fn draw_positive<T: PartialOrd>(name: &str, arr: &[T], value: &T) -> usize {
    draw(name, arr, value, "green")
}

fn draw_negative<T: PartialOrd>(name: &str, arr: &[T], value: &T) -> usize {
    draw(name, arr, value, "red")
}

fn draw<T: PartialOrd>(name: &str, arr: &[T], value: &T, style: &str) -> usize {
    let p = points(arr, value);
    let bar = ProgressBar::with_draw_target(Some(arr.len() as u64), ProgressDrawTarget::stdout());
    bar.set_style(
        ProgressStyle::with_template(&format!(
            "{{msg:13}} {{pos:>2}}/{{len:2}} {{bar:{}.{}}}",
            arr.len(),
            style
        ))
        .unwrap(),
    );
    bar.set_message(Cow::Owned(name.to_owned()));
    bar.set_position(p as u64);
    bar.abandon();
    p
}
